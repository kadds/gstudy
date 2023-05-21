use std::sync::{Arc, Mutex};

use crate::{
    backends::wgpu_backend::{WGPURenderTarget, WGPUResource},
    context::ResourceRef,
};

use super::{pass::PassRenderTargets, resource::ResourceType, ResourceRegistry};

pub struct GraphBackend {
    gpu: Arc<WGPUResource>,
    command_buffers: Arc<Mutex<Vec<wgpu::CommandBuffer>>>,
}

pub struct GraphEncoder {
    w: Option<wgpu::CommandEncoder>,
    targets: Vec<WGPURenderTarget>,
    gpu: Arc<WGPUResource>,
    command_buffers: Arc<Mutex<Vec<wgpu::CommandBuffer>>>,
}

impl GraphEncoder {
    pub fn new_pass<'a>(
        &'a mut self,
        name: &str,
        pass_render_target: &PassRenderTargets,
        registry: &ResourceRegistry,
    ) -> wgpu::RenderPass<'a> {
        let mut render_target = WGPURenderTarget::new("graph target");
        for color in &pass_render_target.colors {
            let (texture_desc, texture) = registry.get_desc_and_underlying(*color);

            if let ResourceType::Texture(info) = &texture_desc.ty {
                render_target.add_render_target(
                    texture.texture_view(),
                    info.clear.as_ref().map(|v| v.color()),
                );
            }
            if let ResourceType::ImportTexture(info) = &texture_desc.ty {
                render_target.add_render_target(
                    texture.texture_view(),
                    info.0.clear.as_ref().map(|v| v.color()),
                );
            }
        }

        if let Some(depth) = &pass_render_target.depth {
            let (texture_desc, texture) = registry.get_desc_and_underlying(*depth);

            if let ResourceType::Texture(info) = &texture_desc.ty {
                render_target.set_depth_target(
                    texture.texture_view(),
                    info.clear.as_ref().map(|v| v.depth()),
                    info.clear.as_ref().map(|v| v.stencil()),
                );
            }
            if let ResourceType::ImportTexture(info) = &texture_desc.ty {
                render_target.set_depth_target(
                    texture.texture_view(),
                    info.0.clear.as_ref().map(|v| v.depth()),
                    info.0.clear.as_ref().map(|v| v.stencil()),
                );
            }
        }

        self.targets.push(render_target);

        self.w
            .as_mut()
            .unwrap()
            .begin_render_pass(self.targets.last().unwrap().desc())
    }
    pub fn encoder(&self) -> &wgpu::CommandEncoder {
        self.w.as_ref().unwrap()
    }
    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.w.as_mut().unwrap()
    }
}

impl Drop for GraphEncoder {
    fn drop(&mut self) {
        let c = self.w.take().unwrap();
        self.command_buffers.lock().unwrap().push(c.finish());
    }
}

impl GraphBackend {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        Self {
            gpu,
            command_buffers: Arc::new(Mutex::new(Vec::new())),
        }
    }
    pub fn create_resource(&self, ty: &ResourceType) -> ResourceRef {
        match ty {
            ResourceType::Texture(t) => {
                if t.size.z == 1 {
                    let tex = self.gpu.device().create_texture(&wgpu::TextureDescriptor {
                        label: None,
                        size: wgpu::Extent3d {
                            width: t.size.x,
                            height: t.size.y,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: t.format,
                        usage: t.usage,
                        view_formats: &[],
                    });
                    self.gpu.context().register_texture(tex)
                } else {
                    let tex = self.gpu.device().create_texture(&wgpu::TextureDescriptor {
                        label: None,
                        size: wgpu::Extent3d {
                            width: t.size.x,
                            height: t.size.y,
                            depth_or_array_layers: t.size.z,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D3,
                        format: t.format,
                        usage: t.usage,
                        view_formats: &[],
                    });
                    self.gpu.context().register_texture(tex)
                }
            }
            ResourceType::Buffer(b) => {
                // let buf = self.gpu.device().create_buffer(&wgpu::BufferDescriptor {
                //     label: None,
                //     size: b.size,
                //     usage: b.usage,
                //     mapped_at_creation: todo!(),
                // });

                // self.gpu.context().register_texture(buf).id()
                todo!()
            }
            ty => panic!("ty {:?}", ty),
        }
    }

    pub fn remove_resource(&self, res: ResourceRef) {
        self.gpu.context().deregister(res);
    }

    pub fn begin_thread(&self) -> GraphEncoder {
        let w = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        GraphEncoder {
            w: Some(w),
            command_buffers: self.command_buffers.clone(),
            gpu: self.gpu.clone(),
            targets: Vec::new(),
        }
    }
    pub fn gpu(&self) -> &WGPUResource {
        &self.gpu
    }
}

impl Drop for GraphBackend {
    fn drop(&mut self) {
        let mut s = self.command_buffers.lock().unwrap();

        let mut tmp = Vec::new();
        std::mem::swap(&mut tmp, &mut *s);
        self.gpu.queue().submit(tmp);
    }
}
