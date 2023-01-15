use std::sync::Arc;

use crate::backends::wgpu_backend::{WGPURenderTarget, WGPURenderer, WGPUResource};

use super::{pass::PassRenderTargets, resource::ResourceType, ResourceRegistry};

pub struct GraphBackend {
    gpu: Arc<WGPUResource>,
}

pub struct GraphEncoder {
    w: WGPURenderer,
    targets: Vec<WGPURenderTarget>,
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
            let (texture_desc, texture_id) = registry.get_desc_and_underlying(*color);
            let texture_ref = self.w.inner.context().get_resource(texture_id.id());

            if let ResourceType::Texture(info) = &texture_desc.ty {
                render_target.add_render_target(
                    texture_ref.texture_view(),
                    info.clear.as_ref().map(|v| v.color()),
                );
            }
            if let ResourceType::ImportTexture(info) = &texture_desc.ty {
                render_target.add_render_target(
                    texture_ref.texture_view(),
                    info.clear.as_ref().map(|v| v.color()),
                );
            }
        }

        if let Some(depth) = &pass_render_target.depth {
            let (texture_desc, texture_id) = registry.get_desc_and_underlying(*depth);

            let texture_ref = self.w.inner.context().get_resource(texture_id.id());
            if let ResourceType::Texture(info) = &texture_desc.ty {
                render_target.set_depth_target(
                    texture_ref.texture_view(),
                    info.clear.as_ref().map(|v| v.depth()),
                );
            }
            if let ResourceType::ImportTexture(info) = &texture_desc.ty {
                render_target.set_depth_target(
                    texture_ref.texture_view(),
                    info.clear.as_ref().map(|v| v.depth()),
                );
            }
        }

        self.targets.push(render_target);

        self.w.new_pass(self.targets.last().unwrap())
    }
    pub fn encoder(&self) -> &wgpu::CommandEncoder {
        &self.w.encoder()
    }
    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.w.encoder_mut()
    }
}

impl GraphBackend {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        Self { gpu }
    }
    pub fn create_resource(&self, ty: &ResourceType) -> crate::ds::DynamicResource {
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

    pub fn begin_thread(&self) -> GraphEncoder {
        GraphEncoder {
            w: WGPURenderer::new(self.gpu.clone()),
            targets: Vec::new(),
        }
    }
}
