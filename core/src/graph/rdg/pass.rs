use std::{
    any::{Any, Provider, TypeId},
    fmt::Debug,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use super::{
    backend::{GraphBackend, GraphEncoder},
    resource::{ClearValue, ResourceOps, RT_COLOR_RESOURCE_ID, RT_DEPTH_RESOURCE_ID},
    PassParameter, RenderGraph, RenderGraphBuilder, ResourceId, ResourceRegistry, ResourceStateMap,
    ResourceUsage,
};

#[derive(Debug, Eq, PartialEq)]
pub enum PreferAttachment {
    Default,
    None,
    Resource(ResourceId),
}

#[derive(Debug)]
pub struct ColorRenderTargetDescriptor {
    pub prefer_attachment: PreferAttachment,
    pub ops: ResourceOps,
}

#[derive(Debug)]
pub struct DepthRenderTargetDescriptor {
    pub prefer_attachment: PreferAttachment,
    pub depth_ops: Option<ResourceOps>,
    pub stencil_ops: Option<ResourceOps>,
}

#[derive(Debug)]
pub struct RenderTargetDescriptor {
    pub colors: smallvec::SmallVec<[ColorRenderTargetDescriptor; 1]>,
    pub depth: Option<DepthRenderTargetDescriptor>,
}

pub struct RenderPassContext<'b> {
    encoder: GraphEncoder,
    state: &'b mut ResourceStateMap,
    registry: &'b ResourceRegistry,
    backend: &'b GraphBackend,
    name: &'b str,
    render_targets: &'b RenderTargetDescriptor,
}

impl<'b> RenderPassContext<'b> {
    pub fn new_pass(&mut self) -> wgpu::RenderPass {
        self.encoder
            .new_pass(self.name, self.render_targets, self.registry, self.state)
    }
}

pub trait RenderPassExecutor {
    fn execute<'b>(&'b mut self, context: RenderPassContext<'b>);
}

pub trait DynPass: Debug {
    fn inputs_outputs(&self) -> (&PassParameter, &PassParameter, &RenderTargetDescriptor);

    fn execute(
        &self,
        registry: &ResourceRegistry,
        backend: &GraphBackend,
        state: &mut ResourceStateMap,
    );

    fn is_default_render_target(&self, id: ResourceId) -> bool {
        false
    }
}

pub struct RenderPass {
    inner: Arc<Mutex<dyn RenderPassExecutor>>,
    pub name: String,
    pub shader_name: String,
    pub inputs: PassParameter,
    pub outputs: PassParameter,
    pub render_targets: RenderTargetDescriptor,
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

    fn execute(
        &self,
        registry: &ResourceRegistry,
        backend: &GraphBackend,
        state: &mut ResourceStateMap,
    ) {
        let mut c = self.inner.lock().unwrap();
        let mut encoder = backend.begin_thread();
        let context = RenderPassContext {
            encoder,
            state,
            registry,
            backend,
            name: &self.name,
            render_targets: &self.render_targets,
        };
        c.execute(context);
    }

    fn is_default_render_target(&self, id: ResourceId) -> bool {
        self.render_targets
            .colors
            .iter()
            .any(|v| v.prefer_attachment == PreferAttachment::Default)
    }
}

pub struct RenderPassBuilder {
    name: String,
    shader_name: String,
    inputs: PassParameter,
    outputs: PassParameter,

    inner: Option<Arc<Mutex<dyn RenderPassExecutor>>>,

    render_targets: Option<RenderTargetDescriptor>,
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
        }
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

    pub fn build(self) -> RenderPass {
        RenderPass {
            inner: self.inner.unwrap(),
            name: self.name,
            shader_name: self.shader_name,
            inputs: self.inputs,
            outputs: self.outputs,
            render_targets: self.render_targets.unwrap(),
        }
    }
}

// pub struct ClearPass {
//     name: String,
//     inputs: PassParameter,
//     outputs: PassParameter,
//     render_targets: PassRenderTargets,
// }

// impl DynPass for ClearPass {
//     fn inputs_outputs(&self) -> (&PassParameter, &PassParameter) {
//         (&self.inputs, &self.outputs)
//     }

//     fn connect_external(
//         &mut self,
//         b: &mut RenderGraphBuilder,
//         present_color: ResourceId,
//         present_depth: ResourceId,
//     ) {
//         let color = b.alias_texture(present_color);
//         let depth = b.alias_texture(present_depth);
//         let color_output = b.alias_texture(present_color);
//         let depth_output = b.alias_texture(present_depth);

//         let targets = PassRenderTargets::new(color, depth);
//         self.inputs
//             .textures
//             .push((color, ResourceUsage::RenderTargetTextureRead));
//         self.inputs
//             .textures
//             .push((depth, ResourceUsage::RenderTargetTextureRead));

//         self.outputs
//             .textures
//             .push((color_output, ResourceUsage::RenderTargetTextureWrite));
//         self.outputs
//             .textures
//             .push((depth_output, ResourceUsage::RenderTargetTextureWrite));
//         self.render_targets = targets;
//     }

//     fn execute(&self, registry: &ResourceRegistry, backend: &GraphBackend) {
//         let mut encoder = backend.begin_thread();
//         let t = encoder.new_pass("clear pass", &self.render_targets, registry);
//     }
// }

// impl Debug for ClearPass {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("RenderPass")
//             .field("name", &self.name)
//             .finish()
//     }
// }

// #[derive(Debug)]
// pub struct ClearPassBuilder {
//     name: String,
// }

// impl ClearPassBuilder {
//     pub fn new<S: Into<String>>(name: S) -> Self {
//         Self { name: name.into() }
//     }
//     pub fn build(mut self) -> ClearPass {
//         ClearPass {
//             name: self.name,
//             inputs: PassParameter::default(),
//             outputs: PassParameter::default(),
//             render_targets: PassRenderTargets::default(),
//         }
//     }
// }
