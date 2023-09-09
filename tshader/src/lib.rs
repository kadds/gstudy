use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use serde_derive::Deserialize;
use tshader_builder::compiler::ShaderTechCompiler;

pub use tshader_builder::compiler::variants_name;

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

#[derive(Debug, Default)]
pub struct Pass {
    pub name: String,
    pub vs: Option<Shader>,
    pub fs: Option<Shader>,
    pub cs: Option<Shader>,

    pub input_layout: BTreeMap<ResourcePosition, (bool, wgpu::VertexFormat)>,

    pub constants: Vec<wgpu::PushConstantRange>,
    pub bind_layout: BTreeMap<ResourcePosition, wgpu::BindGroupLayoutEntry>,
}

#[derive(Debug, Hash, Eq, PartialEq)]
struct VariantKey {
    name: String,
    index: u32,
}

#[derive(Debug)]
pub struct ShaderTech {
    compiler: ShaderTechCompiler,
    pub name: String,
    variants_map: Mutex<HashMap<VariantKey, Arc<Pass>>>,
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
                            naga::ImageClass::Sampled { kind: _, multi } => {
                                multisampled = multi;
                                wgpu::TextureSampleType::Float { filterable: true }
                            }
                            naga::ImageClass::Depth { multi } => {
                                multisampled = multi;
                                wgpu::TextureSampleType::Depth
                            }
                            naga::ImageClass::Storage {
                                format: _,
                                access: _,
                            } => {
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
                        if comparison {
                            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)
                        } else {
                            wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
                        }
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
            naga::TypeInner::Scalar { kind, width } => Self::to_vertex_format2(kind, *width)?,
            naga::TypeInner::Vector { size, kind, width } => {
                match Self::to_vertex_format2(kind, *width)? {
                    wgpu::VertexFormat::Float32 => match size {
                        naga::VectorSize::Bi => wgpu::VertexFormat::Float32x2,
                        naga::VectorSize::Tri => wgpu::VertexFormat::Float32x3,
                        naga::VectorSize::Quad => wgpu::VertexFormat::Float32x4,
                    },
                    wgpu::VertexFormat::Uint32 => match size {
                        naga::VectorSize::Bi => wgpu::VertexFormat::Uint32x3,
                        naga::VectorSize::Tri => wgpu::VertexFormat::Uint32x3,
                        naga::VectorSize::Quad => wgpu::VertexFormat::Uint32x4,
                    },
                    wgpu::VertexFormat::Sint32 => match size {
                        naga::VectorSize::Bi => wgpu::VertexFormat::Sint32x3,
                        naga::VectorSize::Tri => wgpu::VertexFormat::Sint32x3,
                        naga::VectorSize::Quad => wgpu::VertexFormat::Sint32x4,
                    },
                    wgpu::VertexFormat::Float64 => match size {
                        naga::VectorSize::Bi => wgpu::VertexFormat::Float64x2,
                        naga::VectorSize::Tri => wgpu::VertexFormat::Float64x3,
                        naga::VectorSize::Quad => wgpu::VertexFormat::Float64x4,
                    },
                    _ => anyhow::bail!("vertex format is not supported"),
                }
            }
            naga::TypeInner::Struct {
                members: _,
                span: _,
            } => {
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
        name: &str,
        ty: &naga::Type,
        _module: &naga::Module,
        layout: &mut BTreeMap<ResourcePosition, (bool, wgpu::VertexFormat)>,
        new_group: &mut bool,
    ) -> anyhow::Result<()> {
        match binding {
            naga::Binding::BuiltIn(_builtin) => {
                anyhow::bail!("unsupported vertex input binding")
            }
            naga::Binding::Location {
                location,
                interpolation: _,
                sampling: _,
            } => {
                let format = Self::to_vertex_format(ty)?;
                if let naga::TypeInner::Vector {
                    size,
                    kind: _,
                    width: _,
                } = ty.inner
                {
                    if location == 0
                        && (size == naga::VectorSize::Quad || size == naga::VectorSize::Tri)
                    {
                        *new_group = true;
                        layout.insert(ResourcePosition::new(0, 0), (false, format));
                        return Ok(());
                    }
                }
                let is_instance = name.starts_with("instance");
                if *new_group {
                    layout.insert(ResourcePosition::new(1, location), (is_instance, format))
                } else {
                    layout.insert(ResourcePosition::new(0, location), (is_instance, format))
                }
            }
        };
        Ok(())
    }

    fn inputs_to_layouts(
        args: &[naga::FunctionArgument],
        module: &naga::Module,
        layout: &mut BTreeMap<ResourcePosition, (bool, wgpu::VertexFormat)>,
    ) -> anyhow::Result<()> {
        let mut new_group = false;

        for arg in args {
            let ty = module.types.get_handle(arg.ty)?;
            if let Some(binding) = &arg.binding {
                Self::input_var_to_layouts(
                    binding.clone(),
                    arg.name.as_ref().map(|v| v.as_str()).unwrap_or_default(),
                    ty,
                    module,
                    layout,
                    &mut new_group,
                )?;
            } else {
                match &ty.inner {
                    naga::TypeInner::Struct { members, span: _ } => {
                        for member in members {
                            let binding = member.binding.clone().ok_or_else(|| {
                                anyhow::anyhow!("not binding found at {:?}", ty.name)
                            })?;
                            let ty = module.types.get_handle(member.ty)?;
                            Self::input_var_to_layouts(
                                binding,
                                member.name.as_ref().map(|v| v.as_str()).unwrap_or_default(),
                                ty,
                                module,
                                layout,
                                &mut new_group,
                            )?
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

        for (_, global_value) in module.global_variables.iter() {
            let h = module.types.get_handle(global_value.ty)?;
            if let Some(pos) = &global_value.binding {
                let pos = ResourcePosition::new(pos.group, pos.binding);
                let ty = Self::type_to_wgpu(h, global_value.space)?;
                let layout = wgpu::BindGroupLayoutEntry {
                    binding: pos.binding,
                    visibility: wgpu::ShaderStages::all(),
                    ty,
                    count: None,
                };
                pass.bind_layout.insert(pos, layout);
            } else if let naga::AddressSpace::PushConstant = global_value.space {
                let size = h.inner.size(module.to_ctx());
                pass.constants.push(wgpu::PushConstantRange {
                    stages: wgpu::ShaderStages::all(),
                    range: 0..size,
                });
            }
        }

        Ok(pass)
    }

    pub fn register_variant_pass(
        &self,
        device: &wgpu::Device,
        index: usize,
        variants: &[&'static str],
    ) -> anyhow::Result<Arc<Pass>> {
        let mut l = self.variants_map.lock().unwrap();

        let key = VariantKey {
            name: variants_name(variants),
            index: index as u32,
        };
        if let Some(v) = l.get(&key) {
            return Ok(v.clone());
        }

        let shader_descriptor = self.compiler.compile_pass(index, variants)?;
        // create vs, fs, cs
        let label = format!("{}-{}", self.name, index);

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&label),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_descriptor.source)),
        });
        let module = naga::front::wgsl::parse_str(&shader_descriptor.source)?;
        let mut pass = Self::parse_pass(&shader_descriptor.include_shaders, module, shader_module)?;
        pass.name = label;
        log::info!(
            "load variant {:?} for tech '{}', pass: {:?}",
            key,
            self.name,
            pass
        );

        let pass = Arc::new(pass);
        l.insert(key, pass.clone());
        Ok(pass)
    }

    pub fn register_variant(
        &self,
        device: &wgpu::Device,
        variants_list: &[&[&'static str]],
    ) -> anyhow::Result<Vec<Arc<Pass>>> {
        let mut pass_list = vec![];
        let mut l = self.variants_map.lock().unwrap();

        for pass_index in 0..self.compiler.npass() {
            let key = VariantKey {
                name: variants_name(variants_list[pass_index]),
                index: pass_index as u32,
            };
            if let Some(v) = l.get(&key) {
                pass_list.push(v.clone());
                continue;
            }

            let shader_descriptor = self
                .compiler
                .compile_pass(pass_index, variants_list[pass_index])?;
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
            log::info!(
                "load variant {:?} for tech '{}', pass: {:?}",
                key,
                self.name,
                pass
            );

            let pass = Arc::new(pass);
            l.insert(key, pass.clone());

            pass_list.push(pass);
        }

        Ok(pass_list)
    }
}

pub struct Loader {
    desc_path: PathBuf,
    desc_config: Desc,
    tech_map: Mutex<HashMap<LoadTechConfig, Arc<ShaderTech>>>,
}

#[derive(Debug, Deserialize)]
struct Desc {
    map: HashMap<String, String>,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct LoadTechConfig {
    pub name: String,
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

    pub fn load_tech(&self, config: LoadTechConfig) -> anyhow::Result<Arc<ShaderTech>> {
        let mut map = self.tech_map.lock().unwrap();
        if !map.contains_key(&config) {
            let path_component = self
                .desc_config
                .map
                .get(&config.name)
                .ok_or_else(|| anyhow::anyhow!("shader tech {} not found", config.name))?;
            let base_path = PathBuf::from(&self.desc_path)
                .canonicalize()?
                .parent()
                .unwrap()
                .to_path_buf();

            let compiler = ShaderTechCompiler::new(path_component, base_path)?;
            let tech = ShaderTech {
                compiler,
                name: config.name.to_owned(),
                variants_map: Mutex::new(HashMap::new()),
            };
            map.insert(config.clone(), Arc::new(tech));
        }

        Ok(map.get(&config).cloned().unwrap())
    }
}
