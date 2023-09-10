use std::{
    any::Any,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use crate::backends::wgpu_backend::{ClearValue, ResourceOps, WGPUResource};

use super::{
    backend::{GraphBackend, GraphCopyEngine, GraphRenderEngine},
    resource::{RT_COLOR_RESOURCE_ID, RT_DEPTH_RESOURCE_ID},
    PassParameter, ResourceId, ResourceRegistry, ResourceUsage,
};

#[derive(Debug)]
pub enum RenderStage {
    Prepare, // external texture copies... samplers

    Queue, // bind group

    Render, //

    Cleanup,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum PreferAttachment {
    Default,
    None,
    Resource(ResourceId),
}

#[derive(Debug, Clone)]
pub struct ColorRenderTargetDescriptor {
    pub prefer_attachment: PreferAttachment,
    pub resolve_attachment: PreferAttachment,
    pub ops: ResourceOps,
}

#[derive(Debug, Clone)]
pub struct DepthRenderTargetDescriptor {
    pub prefer_attachment: PreferAttachment,
    pub depth_ops: Option<ResourceOps>,
    pub stencil_ops: Option<ResourceOps>,
}

#[derive(Debug, Clone)]
pub struct RenderTargetDescriptor {
    pub colors: smallvec::SmallVec<[ColorRenderTargetDescriptor; 1]>,
    pub depth: Option<DepthRenderTargetDescriptor>,
}

impl RenderTargetDescriptor {
    pub fn has_default(&self) -> bool {
        for c in &self.colors {
            if PreferAttachment::Default == c.prefer_attachment {
                return true;
            }
        }
        if let Some(depth) = &self.depth {
            if PreferAttachment::Default == depth.prefer_attachment {
                return true;
            }
        }
        false
    }

    pub fn new_color(color: ColorRenderTargetDescriptor) -> Self {
        Self {
            colors: smallvec::smallvec![color],
            depth: None,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct RenderTargetState {
    pub colors: bitmaps::Bitmap<64>,
    pub depth: Option<bool>,
    pub msaa: u32,
}

impl RenderTargetState {
    pub fn new(desc: &RenderTargetDescriptor, res_id_list: &[ResourceId], msaa: u32) -> Self {
        let mut t = Self::default();
        t.msaa = msaa;

        for (index, c) in desc.colors.iter().enumerate() {
            let res_id = match c.prefer_attachment {
                PreferAttachment::Default => RT_COLOR_RESOURCE_ID,
                PreferAttachment::Resource(res) => res,
                _ => continue,
            };
            if res_id_list.contains(&res_id) {
                t.colors.set(index, true);
            } else {
                t.colors.set(index, false);
            }
        }

        if let Some(depth) = &desc.depth {
            let res_id = match depth.prefer_attachment {
                PreferAttachment::Default => RT_DEPTH_RESOURCE_ID,
                PreferAttachment::Resource(res) => res,
                _ => return t,
            };
            t.depth = Some(res_id_list.contains(&res_id))
        }

        t
    }

    pub fn color(
        &self,
        index: usize,
        desc: &RenderTargetDescriptor,
        texture_clear: &Option<ClearValue>,
    ) -> Option<ResourceOps> {
        if desc.colors[index].ops.load.is_some() {
            return Some(desc.colors[index].ops.clone());
        } else {
            if self.colors.get(index) {
                if let Some(color) = texture_clear {
                    return Some(ResourceOps {
                        load: Some(color.clone()),
                        store: desc.colors[index].ops.store,
                    });
                }
            }
        }
        return Some(ResourceOps {
            load: None,
            store: true,
        });
        // None
    }

    pub fn depth(
        &self,
        desc: &RenderTargetDescriptor,
        texture_clear: &Option<ClearValue>,
    ) -> (Option<ResourceOps>, Option<ResourceOps>) {
        if let Some(p) = &desc.depth {
            let mut depth = None;
            let mut stencil = None;
            if self.depth.unwrap_or_default() {
                depth = texture_clear
                    .as_ref()
                    .and_then(|v| v.depth())
                    .map(|v| ResourceOps {
                        load: Some(ClearValue::Depth(v)),
                        store: true,
                    });
                stencil = texture_clear
                    .as_ref()
                    .and_then(|v| v.stencil())
                    .map(|v| ResourceOps {
                        load: Some(ClearValue::Stencil(v)),
                        store: true,
                    });
            }
            if depth.is_none() {
                depth = p.depth_ops.clone();
            }
            if stencil.is_none() {
                stencil = p.stencil_ops.clone();
            }

            // if let Some(depth) = &mut depth {
            //     if let Some(stencil) = &mut stencil {
            //     } else {
            //     }
            // } else {
            //     if let Some(stencil) = &mut stencil {
            //     } else {
            //     }
            // }
            return (depth, stencil);
        }
        (None, None)
    }
}

#[derive(Clone, Copy)]
pub struct RenderPassContext<'b> {
    name: &'b str,
    parameter: &'b dyn Any,
    gpu: &'b WGPUResource,
    pub registry: &'b ResourceRegistry,
}

impl<'b> RenderPassContext<'b> {
    pub fn take<T: 'static + Send + Sync>(&self) -> &T {
        self.parameter.downcast_ref::<T>().unwrap()
    }
}

pub trait RenderPassExecutor {
    fn prepare<'b>(
        &'b mut self,
        context: RenderPassContext<'b>,
        engine: &mut GraphCopyEngine,
    ) -> Option<()>;
    fn queue<'b>(&'b mut self, context: RenderPassContext<'b>, device: &wgpu::Device);
    fn render<'b>(&'b mut self, context: RenderPassContext<'b>, engine: &mut GraphRenderEngine);
    fn cleanup<'b>(&'b mut self, context: RenderPassContext<'b>);
}

pub trait DynPass: Debug {
    fn inputs_outputs(&self) -> (&PassParameter, &PassParameter, &RenderTargetDescriptor);

    fn execute(
        &self,
        registry: &ResourceRegistry,
        backend: &GraphBackend,
        render_target_state: &RenderTargetState,
        context: &dyn Any,
    );

    fn name(&self) -> &str;

    fn is_default_render_target(&self, _id: ResourceId) -> bool {
        false
    }

    fn constraints(&self) -> Option<&PassConstraints> {
        None
    }
}

pub struct RenderPass {
    inner: Arc<Mutex<dyn RenderPassExecutor>>,
    pub name: String,
    pub shader_name: String,
    pub inputs: PassParameter,
    pub outputs: PassParameter,
    pub render_targets: RenderTargetDescriptor,
    pub constraints: PassConstraints,
}

impl Debug for RenderPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderPass")
            .field("name", &self.name)
            .finish()
    }
}

impl DynPass for RenderPass {
    fn inputs_outputs(&self) -> (&PassParameter, &PassParameter, &RenderTargetDescriptor) {
        (&self.inputs, &self.outputs, &self.render_targets)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn execute(
        &self,
        registry: &ResourceRegistry,
        backend: &GraphBackend,
        render_target_state: &RenderTargetState,
        parameter: &dyn Any,
    ) {
        profiling::scope!(&self.name);

        let mut c = self.inner.lock().unwrap();
        let context = RenderPassContext {
            name: &self.name,
            parameter,
            gpu: backend.gpu(),
            registry: registry,
        };
        {
            profiling::scope!("copy engine");
            let mut copy_engine = backend.dispatch_copy(&self.name);
            if c.prepare(context, &mut copy_engine).is_none() {
                // clear target
                let mut render_engine = backend.dispatch_render(
                    &self.name,
                    &self.render_targets,
                    render_target_state,
                    registry,
                );
                render_engine.begin(0);
                return;
            }
        }
        {
            profiling::scope!("queue engine");
            c.queue(context, backend.gpu().device());
        }

        {
            profiling::scope!("render engine");
            let mut render_engine = backend.dispatch_render(
                &self.name,
                &self.render_targets,
                render_target_state,
                registry,
            );
            c.render(context, &mut render_engine);
        }
        c.cleanup(context);
    }

    fn is_default_render_target(&self, _id: ResourceId) -> bool {
        self.render_targets
            .colors
            .iter()
            .any(|v| v.prefer_attachment == PreferAttachment::Default)
    }

    fn constraints(&self) -> Option<&PassConstraints> {
        Some(&self.constraints)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PassConstraint {
    Before(String),
    After(String),
    First,
    Last,
}

#[derive(Debug, Default)]
pub struct PassConstraints {
    pub constraints: Vec<PassConstraint>,
}

impl PassConstraints {
    pub fn add(&mut self, o: &Self) {
        self.constraints.extend_from_slice(&o.constraints);
    }
    pub fn has_last(&self) -> bool {
        self.constraints.contains(&PassConstraint::Last)
    }
    pub fn has_first(&self) -> bool {
        self.constraints.contains(&PassConstraint::First)
    }
}

pub struct RenderPassBuilder {
    pub(crate) name: String,
    shader_name: String,
    inputs: PassParameter,
    outputs: PassParameter,

    inner: Option<Arc<Mutex<dyn RenderPassExecutor>>>,

    render_targets: Option<RenderTargetDescriptor>,
    pub(crate) constraints: PassConstraints,
}

impl RenderPassBuilder {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            shader_name: String::new(),
            inputs: PassParameter::default(),
            outputs: PassParameter::default(),
            inner: None,
            render_targets: None,
            constraints: PassConstraints::default(),
        }
    }

    pub fn add_constraint(&mut self, constraint: PassConstraint) {
        self.constraints.constraints.push(constraint);
    }

    pub fn read_texture(&mut self, input: ResourceId) {
        self.inputs
            .textures
            .push((input, ResourceUsage::TextureRead))
    }

    pub fn write_texture(&mut self, output: ResourceId) {
        self.outputs
            .textures
            .push((output, ResourceUsage::TextureWrite))
    }

    pub fn read_buffer(&mut self, input: ResourceId) {
        self.inputs.buffers.push((input, ResourceUsage::BufferRead))
    }

    pub fn read_write_texture(&mut self, t: ResourceId) {
        self.inputs
            .textures
            .push((t, ResourceUsage::TextureReadAndWrite));
        self.outputs
            .textures
            .push((t, ResourceUsage::TextureReadAndWrite));
    }

    pub fn set_shader_name<S: Into<String>>(&mut self, shader_name: S) {
        self.shader_name = shader_name.into();
    }

    pub fn async_execute(&mut self, exec: Arc<Mutex<dyn RenderPassExecutor>>) {
        self.inner = Some(exec);
    }

    pub fn render_target(&mut self, desc: RenderTargetDescriptor) {
        self.render_targets = Some(desc);
    }

    pub fn default_color_depth_render_target(&mut self) {
        let desc = RenderTargetDescriptor {
            colors: smallvec::smallvec![ColorRenderTargetDescriptor {
                prefer_attachment: PreferAttachment::Default,
                resolve_attachment: PreferAttachment::Default,
                ops: ResourceOps {
                    load: None,
                    store: true,
                },
            }],
            depth: Some(DepthRenderTargetDescriptor {
                prefer_attachment: PreferAttachment::Default,
                depth_ops: Some(ResourceOps {
                    load: None,
                    store: true,
                }),
                stencil_ops: None,
            }),
        };
        self.render_target(desc);
    }

    pub fn build(self) -> RenderPass {
        RenderPass {
            inner: self.inner.unwrap(),
            name: self.name,
            shader_name: self.shader_name,
            inputs: self.inputs,
            outputs: self.outputs,
            render_targets: self.render_targets.unwrap(),
            constraints: self.constraints,
        }
    }
}

pub struct ClearPass {
    name: String,
    inputs: PassParameter,
    outputs: PassParameter,
    render_targets: RenderTargetDescriptor,
}

impl DynPass for ClearPass {
    fn inputs_outputs(&self) -> (&PassParameter, &PassParameter, &RenderTargetDescriptor) {
        (&self.inputs, &self.outputs, &self.render_targets)
    }

    fn name(&self) -> &str {
        &self.name
    }

    #[profiling::function]
    fn execute(
        &self,
        registry: &ResourceRegistry,
        backend: &GraphBackend,
        _render_target_state: &RenderTargetState,
        _context: &dyn Any,
    ) {
        let mut r = backend.dispatch_render_with_clear(&self.name, &self.render_targets, registry);
        r.begin(0);
    }
}

impl Debug for ClearPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderPass")
            .field("name", &self.name)
            .finish()
    }
}

#[derive(Debug)]
pub struct ClearPassBuilder {
    name: String,
    render_targets: Option<RenderTargetDescriptor>,
}

impl ClearPassBuilder {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            render_targets: None,
        }
    }
    pub fn render_target(mut self, desc: RenderTargetDescriptor) -> Self {
        self.render_targets = Some(desc);
        self
    }
    pub fn build(self) -> ClearPass {
        ClearPass {
            name: self.name,
            inputs: PassParameter::default(),
            outputs: PassParameter::default(),
            render_targets: self.render_targets.unwrap(),
        }
    }
}
