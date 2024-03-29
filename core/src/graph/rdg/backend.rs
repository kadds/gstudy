use std::{
    cell::RefCell,
    sync::{mpsc, Arc},
};

use crate::{
    backends::wgpu_backend::{ClearValue, ResourceOps, WGPURenderTarget, WGPUResource},
    context::ResourceRef,
};

use super::{
    pass::{RenderTargetDescriptor, RenderTargetState},
    resource::*,
    ResourceRegistry,
};
pub struct GraphBackend {
    gpu: Arc<WGPUResource>,
    rx: mpsc::Receiver<(wgpu::CommandBuffer, u32)>,
    tx: mpsc::Sender<(wgpu::CommandBuffer, u32)>,
}

impl GraphBackend {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        let (tx, rx) = mpsc::channel();
        Self { gpu, tx, rx }
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
                        sample_count: t.sampler_count,
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
                        sample_count: t.sampler_count,
                        dimension: wgpu::TextureDimension::D3,
                        format: t.format,
                        usage: t.usage,
                        view_formats: &[],
                    });
                    self.gpu.context().register_texture(tex)
                }
            }
            ResourceType::Buffer(_b) => {
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

    pub fn dispatch_render_with_clear<'a>(
        &self,
        name: &str,
        pass_render_target: &'a RenderTargetDescriptor,
        registry: &'a ResourceRegistry,
    ) -> GraphRenderEngine {
        let mut render_target = WGPURenderTarget::new(name);
        let mut render_target2 = WGPURenderTarget::new(name);
        for color in &pass_render_target.colors {
            let res_id = match color.prefer_attachment {
                super::pass::PreferAttachment::Default => RT_COLOR_RESOURCE_ID,
                super::pass::PreferAttachment::None => continue,
                super::pass::PreferAttachment::Resource(r) => r,
            };
            let (texture, texture_desc) = registry.get_underlying(res_id);

            if let Some(c) = match &texture_desc.inner {
                ResourceType::Texture(info) => Some(&info.clear),
                ResourceType::ImportTexture(info) => Some(&info.clear),
                _ => None,
            } {
                render_target2.add_render_target(
                    texture.texture_view(),
                    c.as_ref().map(|_| ResourceOps::load_store()),
                );
                render_target.add_render_target(
                    texture.texture_view(),
                    c.as_ref().and_then(|v| v.color()).map(|c| ResourceOps {
                        load: Some(ClearValue::Color(c)),
                        store: true,
                    }),
                );
            }
        }
        if let Some(depth) = &pass_render_target.depth {
            let res_id = match depth.prefer_attachment {
                super::pass::PreferAttachment::Default => Some(RT_DEPTH_RESOURCE_ID),
                super::pass::PreferAttachment::None => None,
                super::pass::PreferAttachment::Resource(r) => Some(r),
            };
            if let Some(res_id) = res_id {
                let (texture, texture_desc) = registry.get_underlying(res_id);
                if let Some(c) = match &texture_desc.inner {
                    ResourceType::Texture(info) => Some(&info.clear),
                    ResourceType::ImportTexture(info) => Some(&info.clear),
                    _ => None,
                } {
                    let depth = c.as_ref().and_then(|v| v.depth()).map(|v| ResourceOps {
                        load: Some(ClearValue::Depth(v)),
                        store: true,
                    });
                    let stencil = c.as_ref().and_then(|v| v.stencil()).map(|v| ResourceOps {
                        load: Some(ClearValue::Stencil(v)),
                        store: true,
                    });
                    render_target2.set_depth_target(
                        texture.texture_view(),
                        depth.as_ref().map(|_| ResourceOps::load_store()),
                        stencil.as_ref().map(|_| ResourceOps::load_store()),
                    );
                    render_target.set_depth_target(texture.texture_view(), depth, stencil);
                }
            }
        }

        GraphRenderEngine {
            gpu: self.gpu.clone(),
            ws: vec![],
            cb: vec![],
            render_target,
            render_target2,
        }
    }

    pub fn dispatch_render<'a>(
        &self,
        name: &str,
        pass_render_target: &'a RenderTargetDescriptor,
        render_target_state: &'a RenderTargetState,
        registry: &'a ResourceRegistry,
    ) -> GraphRenderEngine {
        let mut render_target = WGPURenderTarget::new(name);
        let mut render_target2 = WGPURenderTarget::new(name);
        for (index, color) in pass_render_target.colors.iter().enumerate() {
            let res_id = match color.prefer_attachment {
                super::pass::PreferAttachment::Default => RT_COLOR_RESOURCE_ID,
                super::pass::PreferAttachment::None => continue,
                super::pass::PreferAttachment::Resource(r) => r,
            };
            let (texture, texture_desc) = registry.get_underlying(res_id);

            if let Some(c) = match &texture_desc.inner {
                ResourceType::Texture(info) => Some(&info.clear),
                ResourceType::ImportTexture(info) => Some(&info.clear),
                _ => None,
            } {
                let color_ops = render_target_state.color(index, pass_render_target, c);
                if render_target_state.msaa > 1 {
                    let resolve_res_id = match color.resolve_attachment {
                        super::pass::PreferAttachment::Default => RT_RESOLVE_COLOR_RESOURCE_ID,
                        super::pass::PreferAttachment::None => continue,
                        super::pass::PreferAttachment::Resource(r) => r,
                    };
                    let (resolve_texture, _) = registry.get_underlying(resolve_res_id);
                    let rtv = Some(texture.texture_view());
                    let rt = resolve_texture.texture_view();
                    render_target2.add_resolved_render_target(
                        rt,
                        rtv,
                        color_ops.as_ref().map(|_| ResourceOps::load_store()),
                    );
                    render_target.add_resolved_render_target(rt, rtv, color_ops);
                } else {
                    render_target2.add_resolved_render_target(
                        texture.texture_view(),
                        None,
                        color_ops.as_ref().map(|_| ResourceOps::load_store()),
                    );
                    render_target.add_resolved_render_target(
                        texture.texture_view(),
                        None,
                        color_ops,
                    );
                }
            }
        }
        if let Some(depth) = &pass_render_target.depth {
            let res_id = match depth.prefer_attachment {
                super::pass::PreferAttachment::Default => Some(RT_DEPTH_RESOURCE_ID),
                super::pass::PreferAttachment::None => None,
                super::pass::PreferAttachment::Resource(r) => Some(r),
            };
            if let Some(res_id) = res_id {
                let (texture, texture_desc) = registry.get_underlying(res_id);
                if let Some(c) = match &texture_desc.inner {
                    ResourceType::Texture(info) => Some(&info.clear),
                    ResourceType::ImportTexture(info) => Some(&info.clear),
                    _ => None,
                } {
                    let (clear_depth, clear_stencil) =
                        render_target_state.depth(pass_render_target, c);

                    render_target2.set_depth_target(
                        texture.texture_view(),
                        clear_depth.as_ref().map(|_| ResourceOps::load_store()),
                        clear_stencil.as_ref().map(|_| ResourceOps::load_store()),
                    );
                    render_target.set_depth_target(
                        texture.texture_view(),
                        clear_depth,
                        clear_stencil,
                    );
                }
            }
        }

        GraphRenderEngine {
            gpu: self.gpu.clone(),
            ws: vec![],
            cb: vec![],
            render_target,
            render_target2,
        }
    }

    pub fn dispatch_compute(&self) {
        // GraphRenderBackend {
        //     gpu: self.gpu.clone(),
        //     tx: self.tx.clone(),
        //     pass_render_target,
        //     registry,
        //     state,
        todo!()
        // }
    }

    pub fn dispatch_copy(&self, name: &str) -> GraphCopyEngine {
        let w = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(&format!("{} copy engine", name)),
            });
        GraphCopyEngine {
            gpu: self.gpu.clone(),
            w: Some(w),
        }
    }

    pub fn gpu(&self) -> &WGPUResource {
        &self.gpu
    }
}

pub struct GraphRenderEngine {
    gpu: Arc<WGPUResource>,
    pub ws: Vec<Box<RefCell<wgpu::CommandEncoder>>>,
    pub cb: Vec<wgpu::CommandBuffer>,
    pub render_target: WGPURenderTarget,
    pub render_target2: WGPURenderTarget,
}

impl GraphRenderEngine {
    pub fn begin(&mut self, layer: u32) -> wgpu::RenderPass {
        let w = self
            .gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some(&format!("layer {}", layer)),
            });
        let w = Box::new(RefCell::new(w));

        self.ws.push(w);
        unsafe {
            if self.ws.len() == 1 {
                std::mem::transmute(
                    self.ws
                        .last_mut()
                        .unwrap()
                        .borrow_mut()
                        .begin_render_pass(self.render_target.desc()),
                )
            } else {
                std::mem::transmute(
                    self.ws
                        .last_mut()
                        .unwrap()
                        .borrow_mut()
                        .begin_render_pass(self.render_target2.desc()),
                )
            }
        }
    }

    pub fn insert_command_buffers(&mut self, index: usize, command: wgpu::CommandBuffer) {
        self.cb.insert(index, command);
    }
}

impl Drop for GraphRenderEngine {
    fn drop(&mut self) {
        let mut tmp = vec![];
        std::mem::swap(&mut tmp, &mut self.ws);

        let mut commands = vec![];
        std::mem::swap(&mut self.cb, &mut commands);

        for encoder in tmp {
            let encoder = encoder.into_inner();
            commands.push(encoder.finish())
        }

        self.gpu.queue().submit(commands);
    }
}

pub struct GraphCopyEngine {
    gpu: Arc<WGPUResource>,
    pub w: Option<wgpu::CommandEncoder>,
}

impl GraphCopyEngine {
    pub fn device(&self) -> &wgpu::Device {
        self.gpu.device()
    }
    pub fn gpu(&self) -> &WGPUResource {
        &self.gpu
    }
    pub fn gpu_ref(&self) -> Arc<WGPUResource> {
        self.gpu.clone()
    }
    pub fn encoder(&mut self) -> &mut wgpu::CommandEncoder {
        self.w.as_mut().unwrap()
    }
    pub fn take_command(&mut self) -> wgpu::CommandBuffer {
        self.w.take().unwrap().finish()
    }
}

impl Drop for GraphCopyEngine {
    fn drop(&mut self) {
        if let Some(w) = self.w.take() {
            let command = w.finish();
            self.gpu.queue().submit([command]);
        }
    }
}

pub struct GraphComputeEngine {}

impl GraphComputeEngine {}
