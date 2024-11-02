use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::tech::{
    Builtin, GlobalVariable, InputBinding, Pass, PushConstant, Shader, UniformSampler, UniformStruct, UniformSubVariable, UniformTexture
};
use crate::VariantFlags;
use tshader_builder::compiler::ShaderTechCompiler;
use tshader_builder::preprocessor::Variable;

#[derive(Debug, Default)]
pub struct ShaderPassReflection {}

impl ShaderPassReflection {
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

    fn image_to_sample_type(
        class: naga::ImageClass,
    ) -> anyhow::Result<(wgpu::TextureSampleType, bool)> {
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
        Ok((sample_type, multisampled))
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
            naga::TypeInner::Scalar(s) => Self::to_vertex_format2(&s.kind, s.width)?,
            naga::TypeInner::Vector { size, scalar } => {
                match Self::to_vertex_format2(&scalar.kind, scalar.width)? {
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

    fn size_alignment_of_vec_scalar(size: naga::VectorSize, scalar: &naga::Scalar) -> (u32, u32) {
                match size {
                    naga::VectorSize::Bi => {
                        if scalar.width == 4 {
                            match scalar.kind {
                                naga::ScalarKind::Uint |
                                naga::ScalarKind::Sint |
                                naga::ScalarKind::Float => (8, 8),
                                _ => (0, 0)
                            }
                        } else if scalar.width == 2 {
                            match scalar.kind {
                                naga::ScalarKind::Float => (4, 4),
                                _ => (0, 0)
                            }
                        } else {
                            (0, 0)
                        }
                    }
                    naga::VectorSize::Tri => {
                        if scalar.width == 4 {
                            match scalar.kind {
                                naga::ScalarKind::Uint |
                                naga::ScalarKind::Sint |
                                naga::ScalarKind::Float => (12, 16),
                                _ => (0, 0)
                            }
                        } else if scalar.width == 2 {
                            match scalar.kind {
                                naga::ScalarKind::Float => (6, 8),
                                _ => (0, 0)
                            }
                        } else {
                            (0, 0)
                        }
                    }
                    naga::VectorSize::Quad => {
                        if scalar.width == 4 {
                            match scalar.kind {
                                naga::ScalarKind::Uint |
                                naga::ScalarKind::Sint |
                                naga::ScalarKind::Float => (16, 16),
                                _ => (0, 0)
                            }
                        } else if scalar.width == 2 {
                            match scalar.kind {
                                naga::ScalarKind::Float => (8, 8),
                                _ => (0, 0)
                            }
                        } else {
                            (0, 0)
                        }
                    }
                }
    }

    fn size_alignment_of(
        module: &naga::Module,
        ty: &naga::TypeInner) -> (u32, u32) { // size, alignment
        match ty {
            naga::TypeInner::Scalar(scalar) => {
                match scalar.width {
                    2 => (2, 2),
                    4 => (4, 4),
                    8 => (8, 8),
                    _ => return (0, 0)
                }
            }
            naga::TypeInner::Vector { size, scalar } => {
                Self::size_alignment_of_vec_scalar(*size, scalar)
            }
            naga::TypeInner::Matrix { columns, rows, scalar } => {
                let (s, a) = Self::size_alignment_of_vec_scalar(*rows, scalar);
                if s == 0 {
                    return (0, 0)
                }
                let c = match columns {
                    naga::VectorSize::Bi => 2,
                    naga::VectorSize::Tri => 3,
                    naga::VectorSize::Quad => 4,
                };
                let (fs, fa) = match scalar.width {
                    2 => (2, 2),
                    4 => (4, 4),
                    8 => (8, 8),
                    _ => return (0, 0)
                };

                (s * c * wgpu::util::align_to(fs, fa), a)
            }
            naga::TypeInner::Atomic(scalar) => {
                (4, 4)
            }
            naga::TypeInner::Array { base, size, stride } => {
                let (s, a) = Self::size_alignment_of(module, &module.types.get_handle(*base).unwrap().inner);
                if s == 0 {
                    return (0, 0)
                }
                (s, a) // todo, fix
            }
            _ => (0, 0)
        }
    }


    fn search_sub_variable(
        name_prefix: &mut String,
        var_name: &str,
        module: &naga::Module,
        ty: naga::Handle<naga::Type>,
        offset_ptr: &mut u32,
        max_alignment: &mut u32,
        variables: &mut HashMap<String, UniformSubVariable>,
    ) {

        let ty = module.types.get_handle(ty).unwrap();

        let (ssize, alignment) = Self::size_alignment_of(module, &ty.inner);
        *max_alignment = (*max_alignment).max(alignment);

        let offset = *offset_ptr;
        if alignment != 0 {
            *offset_ptr = wgpu::util::align_to(*offset_ptr + ssize, alignment);
        }
        let v = match &ty.inner {
            naga::TypeInner::Scalar(scalar) => {
                UniformSubVariable {
                    size: ssize,
                    offset,
                } 
            }
            naga::TypeInner::Vector { size, scalar } => {
                UniformSubVariable {
                    size: ssize,
                    offset,
                } 
            }
            naga::TypeInner::Matrix { columns, rows, scalar } => {
                UniformSubVariable {
                    size: ssize,
                    offset,
                } 
            }
            naga::TypeInner::Struct { members, span } => {
                for member in members {
                    let var_name = member.name.as_ref().map(|v| v.as_str()).unwrap();
                    Self::search_sub_variable(name_prefix, var_name, module, member.ty, offset_ptr, max_alignment, variables);
                    *offset_ptr = wgpu::util::align_to(*offset_ptr, *max_alignment);
                }
                return
            }
            _ => unimplemented!()
        };
        variables.insert(var_name.to_string(), v);
    }

    fn take_reference(
        f: &naga::Function,
        module: &naga::Module,
        group_name_map: &HashMap<String, String>,

        res: &mut HashMap<String, GlobalVariable>,
    ) -> anyhow::Result<HashSet<String>> {
        let mut found = HashSet::new();
        for (_, expr) in f.expressions.iter() {
            match expr {
                naga::Expression::GlobalVariable(var) => {
                    let var = module.global_variables.try_get(*var)?;
                    let name = var
                        .name
                        .as_ref()
                        .ok_or(anyhow::anyhow!("empty variable name"))?
                        .to_string();
                    let group_name = group_name_map
                        .get(&name)
                        .ok_or(anyhow::anyhow!("invalid group name"))?
                        .to_owned();

                    match var.space {
                        naga::AddressSpace::Uniform => {
                            let ty = module.types.get_handle(var.ty)?;
                            let size = ty.inner.size(module.to_ctx());
                            let bind = var
                                .binding
                                .as_ref()
                                .ok_or(anyhow::anyhow!("no binding in uniform"))?;
                            
                            let mut sub_variables = HashMap::new();
                            let mut name_prefix = String::new();
                            let mut offset = 0;
                            let mut max_alignment = 0;
                            let var_name = var.name.as_ref().map(|v| v.as_str()).unwrap();
                            name_prefix.push_str(var_name);

                            Self::search_sub_variable(&mut name_prefix, var_name, module, var.ty, &mut offset, &mut max_alignment, &mut sub_variables);
                            let size = wgpu::util::align_to(offset, max_alignment);

                            log::info!("add uniform {} struct {:?}", name, sub_variables);

                            res.insert(
                                name.clone(),
                                GlobalVariable::Struct(UniformStruct {
                                    group: bind.group,
                                    binding: bind.binding,
                                    size,
                                    group_name,
                                    sub_variables,
                                }),
                            );
                            found.insert(name);
                        }
                        naga::AddressSpace::Handle => {
                            let bind = var
                                .binding
                                .as_ref()
                                .ok_or(anyhow::anyhow!("no binding in global image/sampler"))?;
                            let ty = module.types.get_handle(var.ty)?;
                            match ty.inner {
                                naga::TypeInner::Image {
                                    dim,
                                    arrayed,
                                    class,
                                } => {
                                    let dimension = Self::image_to_dimension(dim, arrayed)?;
                                    let (sample_type, multisampled) =
                                        Self::image_to_sample_type(class)?;

                                    res.insert(
                                        name.clone(),
                                        GlobalVariable::Texture(UniformTexture {
                                            group: bind.group,
                                            binding: bind.binding,
                                            group_name,
                                            dimension,
                                            sample_type,
                                            multisampled,
                                        }),
                                    );
                                    found.insert(name);
                                }
                                naga::TypeInner::Sampler { comparison } => {
                                    res.insert(
                                        name.clone(),
                                        GlobalVariable::Sampler(UniformSampler {
                                            group: bind.group,
                                            binding: bind.binding,
                                            group_name,
                                            comparison,
                                        }),
                                    );
                                    found.insert(name);
                                }
                                _ => (),
                            }
                        }
                        naga::AddressSpace::PushConstant => {
                            let ty = module.types.get_handle(var.ty)?;
                            let size = ty.inner.size(module.to_ctx());
                            res.insert(
                                name.clone(),
                                GlobalVariable::PushConstant(PushConstant {
                                    size,
                                    group_name: group_name_map
                                        .get(&name)
                                        .ok_or(anyhow::anyhow!("invalid group name"))?
                                        .to_owned(),
                                }),
                            );
                            found.insert(name);
                        }
                        _ => (),
                    }
                }
                _ => {}
            }
        }
        Ok(found)
    }

    fn take_input(
        f: &naga::Function,
        module: &naga::Module,
        res: &mut HashMap<String, InputBinding>,
    ) -> anyhow::Result<HashSet<String>> {
        let mut ms = HashSet::new();
        let mut name_stack = vec![];
        for arg in &f.arguments {
            let name = arg.name.as_deref().unwrap_or_default();
            name_stack.push(name);
            Self::take_input2(module, arg.ty, &arg.binding, res, &mut name_stack, &mut ms)?;
            name_stack.pop();
        }
        Ok(ms)
    }

    fn take_input2<'a>(
        module: &'a naga::Module,
        ty: naga::Handle<naga::Type>,
        binding: &Option<naga::Binding>,
        res: &mut HashMap<String, InputBinding>,
        name_stack: &mut Vec<&'a str>,
        ms: &mut HashSet<String>,
    ) -> anyhow::Result<()> {
        let ty = module.types.get_handle(ty)?;

        if let Some(binding) = binding {
            let size = ty.inner.size(module.to_ctx());
            let alignment = 0;
            let fullname = name_stack.join(".");
            ms.insert(fullname.clone());
            let format = Self::to_vertex_format(ty)?;
            match binding {
                naga::Binding::BuiltIn(f) => match f {
                    naga::BuiltIn::Position { invariant } => {
                        res.insert(
                            fullname,
                            InputBinding {
                                binding: 0,
                                builtin: Some(Builtin::Position),
                                size,
                                alignment,
                                format,
                            },
                        );
                    }
                    _ => (),
                },
                naga::Binding::Location {
                    location,
                    second_blend_source,
                    interpolation,
                    sampling,
                } => {
                    res.insert(
                        fullname,
                        InputBinding {
                            binding: *location,
                            builtin: None,
                            size,
                            alignment,
                            format,
                        },
                    );
                }
            }
        } else {
            match &ty.inner {
                naga::TypeInner::Struct { members, span: _ } => {
                    for member in members {
                        let name = member.name.as_deref().unwrap_or_default();
                        name_stack.push(name);

                        Self::take_input2(module, member.ty, &member.binding, res, name_stack, ms)?;

                        name_stack.pop();
                    }
                }
                _ => {
                    anyhow::bail!("please set binding for {:?}", name_stack)
                }
            }
        }
        Ok(())
    }

    fn parse_pass(
        entries: &[tshader_builder::compiler::Shader],
        global_struct: &HashMap<String, HashMap<String, Variable>>,
        global_variables: &HashMap<String, HashMap<String, Variable>>,
        module: naga::Module,
        shader_module: wgpu::ShaderModule,
    ) -> anyhow::Result<Pass> {
        let mut pass = Pass::default();
        let shader_module = Arc::new(shader_module);
        let mut g = HashMap::new();
        for (group, variables) in global_variables {
            for variable in variables.keys() {
                g.insert(variable.to_owned(), group.to_owned());
            }
        }

        for shader_entry in entries {
            match shader_entry {
                tshader_builder::compiler::Shader::Vertex => {
                    let entry = module
                        .entry_points
                        .iter()
                        .find(|v| v.stage == naga::ShaderStage::Vertex)
                        .ok_or(anyhow::anyhow!("entry not found"))?;
                    let reference = Self::take_reference(
                        &entry.function,
                        &module,
                        &g,
                        &mut pass.global_variables,
                    )?;

                    pass.vs = Some(Shader {
                        device_module: shader_module.clone(),
                        global_reference: reference,
                    });
                    Self::take_input(&entry.function, &module, &mut pass.local_variables)?;
                }
                tshader_builder::compiler::Shader::Fragment => {
                    let entry = module
                        .entry_points
                        .iter()
                        .find(|v| v.stage == naga::ShaderStage::Fragment)
                        .ok_or(anyhow::anyhow!("entry not found"))?;
                    let reference = Self::take_reference(
                        &entry.function,
                        &module,
                        &g,
                        &mut pass.global_variables,
                    )?;

                    pass.fs = Some(Shader {
                        device_module: shader_module.clone(),
                        global_reference: reference,
                    });
                }
                tshader_builder::compiler::Shader::Compute => {
                    let entry = module
                        .entry_points
                        .iter()
                        .find(|v| v.stage == naga::ShaderStage::Compute)
                        .ok_or(anyhow::anyhow!("entry not found"))?;
                    let reference = Self::take_reference(
                        &entry.function,
                        &module,
                        &g,
                        &mut pass.global_variables,
                    )?;

                    pass.cs = Some(Shader {
                        device_module: shader_module.clone(),
                        global_reference: reference,
                    });
                    Self::take_input(&entry.function, &module, &mut pass.local_variables)?;
                }
            };
        }

        Ok(pass)
    }

    #[profiling::function]
    pub fn reflect(
        &self,
        device: &wgpu::Device,
        variant: &VariantFlags,
        name: &str,
        pass_index: usize,
        compiler: &ShaderTechCompiler,
    ) -> anyhow::Result<Arc<Pass>> {
        let shader_descriptor = compiler.compile_pass(pass_index, &variant.view)?;
        // create vs, fs, cs
        let label = 
            if shader_descriptor.name.is_empty() {
                format!("{}-{}", name, pass_index)
            } else {
                shader_descriptor.name.clone()
            };

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&label),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(&shader_descriptor.source)),
        });

        let mut message = String::new();
        for msg in pollster::block_on(shader_module.get_compilation_info()).messages {
            match msg.message_type {
                wgpu::CompilationMessageType::Error => {
                    message.push_str(format!("error:[{:?}] {}\n", msg.location, &msg.message).as_str());
                }
                wgpu::CompilationMessageType::Warning => {
                    message.push_str(format!("warning:[{:?}] {}\n", msg.location, &msg.message).as_str());
                }
                wgpu::CompilationMessageType::Info => {
                    message.push_str(format!("info:[{:?}] {}\n", msg.location, &msg.message).as_str());
                }
            }
        }

        let module = naga::front::wgsl::parse_str(&shader_descriptor.source)?;
        let mut pass = Self::parse_pass(
            &shader_descriptor.entries,
            &shader_descriptor.struct_variables,
            &shader_descriptor.global_variables,
            module,
            shader_module,
        )?;
        pass.name = label;

        log::info!(
            "load variant {:?} for tech '{}', pass: {:?}, message: {}",
            variant.key(),
            name,
            pass,
            &message
        );

        let pass = Arc::new(pass);

        Ok(pass)
    }
}
