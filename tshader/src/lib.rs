use std::{
    any,
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    ops::Range,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde_derive::Deserialize;
use tshader_builder::compiler::{variants_name, ShaderTechCompiler, Variant};

#[derive(Debug)]
pub struct Shader {
    pub device_module: Arc<wgpu::ShaderModule>,
}

#[derive(Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct ResourcePosition {
    pub group: u32,
    pub binding: u32,
}

impl ResourcePosition {
    pub fn new(group: u32, binding: u32) -> Self {
        Self { group, binding }
    }
}

#[derive(Debug)]
pub struct VertexBufferLayout {}

#[derive(Debug, Default)]
pub struct Pass {
    pub name: String,
    pub vs: Option<Shader>,
    pub fs: Option<Shader>,
    pub cs: Option<Shader>,

    pub input_layout: BTreeMap<ResourcePosition, wgpu::VertexFormat>,

    pub constants: Vec<wgpu::PushConstantRange>,
    pub bind_layout: BTreeMap<ResourcePosition, wgpu::BindGroupLayoutEntry>,
}

#[derive(Debug, Hash, Eq, PartialEq)]
pub struct ShaderTechIdentity {
    pub name: String,
}

#[derive(Debug)]
pub struct ShaderTech {
    compiler: ShaderTechCompiler,
    pub name: String,
    pub variants_map: Mutex<HashMap<String, Arc<Vec<Pass>>>>,
}

impl ShaderTech {
    fn image_to_dimension(
        image: naga::ImageDimension,
        array: bool,
    ) -> anyhow::Result<wgpu::TextureViewDimension> {
        Ok(match image {
            naga::ImageDimension::D1 => {
                if array {
                    anyhow::bail!("d1 texture doesn't support array")
                }
                wgpu::TextureViewDimension::D1
            }
            naga::ImageDimension::D2 => {
                if array {
                    wgpu::TextureViewDimension::D2Array
                } else {
                    wgpu::TextureViewDimension::D2
                }
            }
            naga::ImageDimension::D3 => {
                if array {
                    anyhow::bail!("d3 texture doesn't support array")
                }
                wgpu::TextureViewDimension::D3
            }
            naga::ImageDimension::Cube => {
                if array {
                    wgpu::TextureViewDimension::CubeArray
                } else {
                    wgpu::TextureViewDimension::Cube
                }
            }
        })
    }
    fn type_to_wgpu(
        t: &naga::Type,
        space: naga::AddressSpace,
        size: u32,
    ) -> anyhow::Result<wgpu::BindingType> {
        let res = match space {
            naga::AddressSpace::Uniform => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            naga::AddressSpace::Handle => {
                // image sample
                match t.inner {
                    naga::TypeInner::Image {
                        dim,
                        arrayed,
                        class,
                    } => {
                        let view_dimension = Self::image_to_dimension(dim, arrayed)?;
                        let mut multisampled = false;
                        let sample_type = match class {
                            naga::ImageClass::Sampled { kind, multi } => {
                                multisampled = multi;
                                wgpu::TextureSampleType::Float { filterable: true }
                            }
                            naga::ImageClass::Depth { multi } => {
                                multisampled = multi;
                                wgpu::TextureSampleType::Depth
                            }
                            naga::ImageClass::Storage { format, access } => {
                                anyhow::bail!("storage type unknown")
                            }
                        };
                        wgpu::BindingType::Texture {
                            sample_type,
                            view_dimension,
                            multisampled,
                        }
                    }
                    naga::TypeInner::Sampler { comparison } => {
                        wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                    }
                    _ => {
                        anyhow::bail!("unsupported type in address space Handle var {:?}", t.name)
                    }
                }
            }
            _ => {
                anyhow::bail!("unsupported address space var {:?}", t.name)
            }
        };
        Ok(res)
    }

    fn to_vertex_format2(ty: &naga::ScalarKind, width: u8) -> anyhow::Result<wgpu::VertexFormat> {
        let res = match ty {
            naga::ScalarKind::Sint => {
                if width == 4 {
                    wgpu::VertexFormat::Sint32
                } else {
                    anyhow::bail!("scalar width not supported {}", width)
                }
            }
            naga::ScalarKind::Uint => {
                if width == 4 {
                    wgpu::VertexFormat::Uint32
                } else {
                    anyhow::bail!("scalar width not supported {}", width)
                }
            }
            naga::ScalarKind::Float => {
                if width == 4 {
                    wgpu::VertexFormat::Float32
                } else if width == 8 {
                    wgpu::VertexFormat::Float64
                } else {
                    anyhow::bail!("scalar width not supported {}", width)
                }
            }
            _ => {
                anyhow::bail!("scalar kind not supported")
            }
        };
        Ok(res)
    }

    fn to_vertex_format(ty: &naga::Type) -> anyhow::Result<wgpu::VertexFormat> {
        let res = match &ty.inner {
            naga::TypeInner::Scalar { kind, width } => {
                Self::to_vertex_format2(kind, *width)?
            },
            naga::TypeInner::Vector { size, kind, width } => {
                match Self::to_vertex_format2(kind, *width)? {
                    wgpu::VertexFormat::Float32 => {
                        match size {
                            naga::VectorSize::Bi => {
                                wgpu::VertexFormat::Float32x2
                            },
                            naga::VectorSize::Tri => {
                                wgpu::VertexFormat::Float32x3
                            }
                            naga::VectorSize::Quad =>{
                                wgpu::VertexFormat::Float32x4
                            },
                        }
                    },
                    wgpu::VertexFormat::Uint32 => {
                        match size {
                            naga::VectorSize::Bi => {
                                wgpu::VertexFormat::Uint32x3
                            },
                            naga::VectorSize::Tri => {
                                wgpu::VertexFormat::Uint32x3
                            }
                            naga::VectorSize::Quad =>{
                                wgpu::VertexFormat::Uint32x4
                            },
                        }
                    },
                    wgpu::VertexFormat::Sint32 => {
                        match size {
                            naga::VectorSize::Bi => {
                                wgpu::VertexFormat::Sint32x3
                            },
                            naga::VectorSize::Tri => {
                                wgpu::VertexFormat::Sint32x3
                            }
                            naga::VectorSize::Quad =>{
                                wgpu::VertexFormat::Sint32x4
                            },
                        }
                    },
                    wgpu::VertexFormat::Float64 => {
                        match size {
                            naga::VectorSize::Bi => {
                                wgpu::VertexFormat::Float64x2
                            },
                            naga::VectorSize::Tri => {
                                wgpu::VertexFormat::Float64x3
                            }
                            naga::VectorSize::Quad =>{
                                wgpu::VertexFormat::Float64x4
                            },
                        }
                    },
                    _ => anyhow::bail!("vertex format is not supported")
                }
            }
            naga::TypeInner::Struct { members, span } => {
                anyhow::bail!("struct is not supported")
            }
            _ => {
                anyhow::bail!("vertex type is not supported")
            }
        };
        Ok(res)
    }

    fn input_var_to_layouts(
        binding: naga::Binding,
        ty: &naga::Type,
        module: &naga::Module,
        layout: &mut BTreeMap<ResourcePosition, wgpu::VertexFormat>,
    ) -> anyhow::Result<()> {
        match binding {
            naga::Binding::BuiltIn(builtin) => {
                anyhow::bail!("unsupported vertex input binding")
            }
            naga::Binding::Location {
                location,
                interpolation,
                sampling,
            } => {
                let format = Self::to_vertex_format(ty)?;
                layout.insert(ResourcePosition::new(0, location), format)
            }
        };
        Ok(())
    }

    fn inputs_to_layouts(
        args: &[naga::FunctionArgument],
        module: &naga::Module,
        layout: &mut BTreeMap<ResourcePosition, wgpu::VertexFormat>,
    ) -> anyhow::Result<()> {
        for arg in args {
            let ty = module.types.get_handle(arg.ty)?;
            if let Some(binding) = &arg.binding {
                Self::input_var_to_layouts(binding.clone(), ty, module, layout)?;
            } else {
                match &ty.inner {
                    naga::TypeInner::Struct { members, span } => {
                        for member in members {
                            let binding = member.binding.clone().ok_or_else(|| {
                                anyhow::anyhow!("not binding found at {:?}", ty.name)
                            })?;
                            let ty = module.types.get_handle(member.ty)?;
                            Self::input_var_to_layouts(binding, ty, module, layout)?
                        }
                    }
                    _ => {
                        anyhow::bail!("please set binding for {:?}", arg.name)
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_pass(
        includes: &[tshader_builder::compiler::Shader],
        module: naga::Module,
        shader_module: wgpu::ShaderModule,
    ) -> anyhow::Result<Pass> {
        let mut pass = Pass::default();
        let shader_module = Arc::new(shader_module);

        for shader_ty in includes {
            match shader_ty {
                tshader_builder::compiler::Shader::Vertex => {
                    let entry = module.entry_points.iter().find(|v| v.name == "vs_main");
                    if let Some(entry) = entry {
                        Self::inputs_to_layouts(
                            &entry.function.arguments,
                            &module,
                            &mut pass.input_layout,
                        )?;
                    } else {
                        anyhow::bail!("entry vs_main not found");
                    }
                    pass.vs = Some(Shader {
                        device_module: shader_module.clone(),
                    })
                }
                tshader_builder::compiler::Shader::Fragment => {
                    pass.fs = Some(Shader {
                        device_module: shader_module.clone(),
                    })
                }
                tshader_builder::compiler::Shader::Compute => {
                    pass.cs = Some(Shader {
                        device_module: shader_module.clone(),
                    })
                }
            };
        }

        for (global, global_value) in module.global_variables.iter() {
            if let Some(pos) = &global_value.binding {
                let pos = ResourcePosition::new(pos.group, pos.binding);
                let ty = {
                    let h = module.types.get_handle(global_value.ty)?;
                    let mut size = h.inner.size(&module.constants);
                    match global_value.space {
                        naga::AddressSpace::PushConstant => {
                            if size % 4 != 0 {
                                size += 4 - (size % 4);
                            }
                            pass.constants.push(wgpu::PushConstantRange {
                                stages: wgpu::ShaderStages::all(),
                                range: 0..size,
                            });
                            continue;
                        }
                        _ => Self::type_to_wgpu(h, global_value.space, size),
                    }
                }?;
                let layout = wgpu::BindGroupLayoutEntry {
                    binding: pos.binding,
                    visibility: wgpu::ShaderStages::all(),
                    ty,
                    count: None,
                };
                pass.bind_layout.insert(pos, layout);
            }
        }

        Ok(pass)
    }

    pub fn register_variant(
        &self,
        device: &wgpu::Device,
        variants: &[Variant],
    ) -> anyhow::Result<Arc<Vec<Pass>>> {
        let key = variants_name(variants);
        let mut l = self.variants_map.lock().unwrap();
        let mut pass_list = vec![];

        if !l.contains_key(&key) {
            for pass_index in 0..self.compiler.npass() {
                let shader_descriptor = self.compiler.compile_pass(pass_index, variants)?;
                // create vs, fs, cs
                let label = format!("{}-{}", self.name, pass_index);

                let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some(&label),
                    source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_descriptor.source)),
                });
                let module = naga::front::wgsl::parse_str(&shader_descriptor.source)?;
                let mut pass =
                    Self::parse_pass(&shader_descriptor.include_shaders, module, shader_module)?;
                pass.name = label;
                pass_list.push(pass);
            }
            l.insert(key.clone(), Arc::new(pass_list));
        }

        Ok(l.get(&key).cloned().unwrap())
    }
}

pub struct Loader {
    desc_path: PathBuf,
    desc_config: Desc,
    tech_map: Mutex<HashMap<String, Arc<ShaderTech>>>,
}

#[derive(Debug, Deserialize)]
struct Desc {
    map: HashMap<String, String>,
}

impl Loader {
    pub fn new(desc_path: PathBuf) -> anyhow::Result<Self> {
        let source = std::fs::read_to_string(&desc_path)?;
        let desc_config: Desc = toml::from_str(&source)?;
        Ok(Self {
            desc_path,
            desc_config,
            tech_map: Mutex::new(HashMap::new()),
        })
    }

    pub fn load_tech<S: AsRef<str>>(&self, name: S) -> anyhow::Result<Arc<ShaderTech>> {
        let mut map = self.tech_map.lock().unwrap();
        let name = name.as_ref();
        if !map.contains_key(name) {
            let path_component = self
                .desc_config
                .map
                .get(name)
                .ok_or_else(|| anyhow::anyhow!("shader tech {} not found", name))?;
            let base_path = PathBuf::from(&self.desc_path).canonicalize()?.parent().unwrap().to_path_buf();

            let compiler = ShaderTechCompiler::new(&path_component, &base_path)?;
            let tech = ShaderTech {
                compiler,
                name: name.to_owned(),
                variants_map: Mutex::new(HashMap::new()),
            };
            map.insert(name.to_owned(), Arc::new(tech));
        }

        Ok(map.get(name).cloned().unwrap())
    }
}
