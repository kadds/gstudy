use std::{collections::HashMap, num::NonZeroU64, sync::Arc};

use wgpu::util::DeviceExt;

use crate::{material::bind::{BindingResourceProvider, ShaderBindingResource}, util::any_as_u8_slice};

use super::pso::{BindGroupType, PipelineStateObject};

pub struct SingleBindGroup {
    pub bind_group: wgpu::BindGroup,
    pub offsets: Vec<u32>,
    pub group: u32, // binding group
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct BindGroupKey {
    pub ty: BindGroupType,
    pub s: u64,      // resource_id
    pub idx: String, // pass name
}

pub struct ShaderBindGroupCollection {
    bind_groups: HashMap<BindGroupKey, SingleBindGroup>,
    name: String,
}

impl ShaderBindGroupCollection {
    pub fn new(name: String) -> Self {
        Self {
            name,
            bind_groups: HashMap::new(),
        }
    }

    pub fn setup(
        &mut self,
        device: &wgpu::Device,
        binding: &dyn BindingResourceProvider,
        id: u64,
        pso: Arc<PipelineStateObject>,
    ) {
        let key = BindGroupKey {
            ty: binding.bind_group(),
            s: id,
            idx: pso.pass_name().to_owned(),
        };

        if self.bind_groups.contains_key(&key) {
            return;
        }

        if let Some((layout, uniforms)) = &pso.get_bind_group_layout(key.ty) {
            let mut entries = vec![];
            let label = format!("{} bind group", &self.name);
            let mut offsets = vec![];
            let mut buffers: Vec<Option<Arc<wgpu::Buffer>>> = vec![];
            buffers.resize(128, None);
            let mut buffer_index = 0;

            for (varname, variable) in &uniforms.vars {

                match variable {
                    tshader::tech::GlobalVariable::Struct(s) => {
                        let mut buf = Vec::new();
                        buf.resize(s.size as usize, 0);

                        for (variable_name, variable) in &s.sub_variables {
                            let offset = variable.offset as usize;
                            let size = variable.size as usize;
                            let end_offset = (variable.offset + variable.size) as usize;
                            match binding.query_resource(&variable_name) {
                                ShaderBindingResource::Double(f) => {
                                    if core::mem::size_of_val(&f) <= size {
                                        buf[offset..end_offset].copy_from_slice(any_as_u8_slice(&f));
                                    } else {
                                        log::error!("resource type {}: \"{}\" size mismatch, expect {}, actual {}", varname, variable_name, 
                                            core::mem::size_of_val(&f), size);
                                    }
                                }
                                ShaderBindingResource::Int32(f) => {
                                    if core::mem::size_of_val(&f) <= size {
                                        buf[offset..end_offset].copy_from_slice(any_as_u8_slice(&f));
                                    } else {
                                        log::error!("resource type {}: \"{}\" size mismatch, expect {}, actual {}", varname, variable_name, 
                                            core::mem::size_of_val(&f), size);
                                    }
                                }
                                ShaderBindingResource::Int64(f) => {
                                    if core::mem::size_of_val(&f) <= size {
                                        buf[offset..end_offset].copy_from_slice(any_as_u8_slice(&f));
                                    } else {
                                        log::error!("resource type {}: \"{}\" size mismatch, expect {}, actual {}", varname, variable_name, 
                                            core::mem::size_of_val(&f), size);
                                    }
                                }
                                ShaderBindingResource::Float(f) => {
                                    if core::mem::size_of_val(&f) <= size {
                                        buf[offset..end_offset].copy_from_slice(any_as_u8_slice(&f));
                                    } else {
                                        log::error!("resource type {}: \"{}\" size mismatch, expect {}, actual {}", varname, variable_name, 
                                            core::mem::size_of_val(&f), size);
                                    }
                                }
                                ShaderBindingResource::Float2(f) => {
                                    if core::mem::size_of_val(&f) <= size {
                                        buf[offset..end_offset].copy_from_slice(any_as_u8_slice(&f));
                                    } else {
                                        log::error!("resource type {}: \"{}\" size mismatch, expect {}, actual {}", varname, variable_name, 
                                            core::mem::size_of_val(&f), size);
                                    }
                                }
                                ShaderBindingResource::Float3(f) => {
                                    if core::mem::size_of_val(&f) <= size {
                                        buf[offset..end_offset].copy_from_slice(any_as_u8_slice(&f));
                                    } else {
                                        log::error!("resource type {}: \"{}\" size mismatch, expect {}, actual {}", varname, variable_name, 
                                            core::mem::size_of_val(&f), size);
                                    }
                                }
                                ShaderBindingResource::Float4(f) => {
                                    if core::mem::size_of_val(&f) <= size {
                                        buf[offset..end_offset].copy_from_slice(any_as_u8_slice(&f));
                                    } else {
                                        log::error!("resource type {}: \"{}\" size mismatch, expect {}, actual {}", varname, variable_name, 
                                            core::mem::size_of_val(&f), size);
                                    }
                                }
                                _ => 
                                    log::error!("unsupported material resource type {}, \"{}\" not find", varname, variable_name),
                            }
                        }

                        let buffer =
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(&label),
                                contents: &buf,
                                usage: wgpu::BufferUsages::UNIFORM,
                            });
                        let buffer = Arc::new(buffer);
                        buffers[buffer_index] = Some(buffer.clone());

                        entries.push(wgpu::BindGroupEntry {
                            binding: s.binding,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: unsafe { std::mem::transmute(buffer.as_ref()) },
                                offset: 0,
                                size: NonZeroU64::new(buf.len() as u64),
                            }),
                        });
                        buffer_index += 1;
                        offsets.push(0);
                    }
                    tshader::tech::GlobalVariable::Texture(t) => {
                        if let ShaderBindingResource::Resource(b) = binding.query_resource(varname) {
                            if let crate::context::ResourceTy::Texture((_, view)) = b.ty() {
                                entries.push(wgpu::BindGroupEntry {
                                    binding: t.binding,
                                    resource: wgpu::BindingResource::TextureView(unsafe {
                                        std::mem::transmute(view)
                                    }),
                                });
                            }
                        } else {
                            log::error!(
                                "unsupported texture material resource type, {} not find",
                                varname
                            );
                        }
                    }
                    tshader::tech::GlobalVariable::Sampler(s) => {
                        if let ShaderBindingResource::Resource(b) = binding.query_resource(varname) {
                            let binding = s.binding;
                            if let crate::context::ResourceTy::Sampler(s) = b.ty() {
                                entries.push(wgpu::BindGroupEntry {
                                    binding,
                                    resource: wgpu::BindingResource::Sampler(unsafe {
                                        std::mem::transmute(s)
                                    }),
                                });
                            }
                        } else {
                            log::error!(
                                "unsupported sampler material resource type, {} not find",
                                varname
                            );
                        }
                    }
                    _ => (),
                }
            }

            log::info!(
                "create bind group (entires {}, vars {})",
                entries.len(),
                uniforms.vars.len()
            );
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&label),
                layout,
                entries: &entries,
            });
            let value = SingleBindGroup {
                bind_group,
                group: uniforms.group,
                offsets,
            };
            self.bind_groups.insert(key, value);
        } else {
            // log::warn!("empty material bind group for {}", mat.face().name());
        }
    }

    pub fn bind(
        &mut self,
        pass: &mut wgpu::RenderPass,
        binding: &dyn BindingResourceProvider,
        id: u64,
        pso: &PipelineStateObject,
    ) {
        let key = BindGroupKey {
            ty: binding.bind_group(),
            s: id,
            idx: pso.pass_name().to_owned(),
        };
        if let Some(g) = self.bind_groups.get(&key) {
            pass.set_bind_group(g.group, &g.bind_group, &g.offsets)
        }
    }
}
