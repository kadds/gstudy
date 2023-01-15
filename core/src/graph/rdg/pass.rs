use std::{
    any::{Any, Provider, TypeId},
    fmt::Debug,
    marker::PhantomData,
    sync::Arc,
};

use super::{
    backend::GraphEncoder,
    resource::{ClearValue, RT_COLOR_RESOURCE_ID, RT_DEPTH_RESOURCE_ID},
    PassParameter, RenderGraph, RenderGraphBuilder, ResourceId, ResourceOp, ResourceRegistry,
};

#[derive(Default, Clone)]
pub struct PassRenderTargets {
    pub colors: smallvec::SmallVec<[ResourceId; 1]>,
    pub depth: Option<ResourceId>,
}

impl PassRenderTargets {
    pub fn new(color: ResourceId, depth: ResourceId) -> Self {
        let mut colors = smallvec::SmallVec::new();

        colors.push(color);
        Self {
            colors,
            depth: Some(depth),
        }
    }
}

pub trait RenderPassExecutor {
    fn execute(&self, registry: &ResourceRegistry, pass: &mut wgpu::RenderPass);
}

pub trait DynPass: Debug {
    fn inputs_outputs(&self) -> (&PassParameter, &PassParameter);
    fn connect_external(
        &mut self,
        b: &mut RenderGraphBuilder,
        present_color: ResourceId,
        present_depth: ResourceId,
    );
    fn execute(&self, registry: &ResourceRegistry, encoder: &mut GraphEncoder);
}

pub struct RenderPass {
    pub inner: Arc<dyn RenderPassExecutor>,
    pub name: String,
    pub shader_name: String,
    pub inputs: PassParameter,
    pub outputs: PassParameter,
    pub render_targets: PassRenderTargets,
    pub bind_default: bool,
}

impl Debug for RenderPass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderPass")
            .field("name", &self.name)
            .field("bind_default", &self.bind_default)
            .finish()
    }
}

impl DynPass for RenderPass {
    fn inputs_outputs(&self) -> (&PassParameter, &PassParameter) {
        (&self.inputs, &self.outputs)
    }

    fn connect_external(
        &mut self,
        b: &mut RenderGraphBuilder,
        present_color: ResourceId,
        present_depth: ResourceId,
    ) {
        if self.bind_default {
            let color = b.alias_texture(present_color);
            let depth = b.alias_texture(present_depth);
            let color_output = b.alias_texture(present_color);
            let depth_output = b.alias_texture(present_depth);

            let targets = PassRenderTargets::new(color, depth);
            self.inputs
                .textures
                .push((color, ResourceOp::RenderTargetTextureRead));
            self.inputs
                .textures
                .push((depth, ResourceOp::RenderTargetTextureRead));

            self.outputs
                .textures
                .push((color_output, ResourceOp::RenderTargetTextureWrite));
            self.outputs
                .textures
                .push((depth_output, ResourceOp::RenderTargetTextureWrite));
            self.render_targets = targets;
        }
    }

    fn execute(&self, registry: &ResourceRegistry, encoder: &mut GraphEncoder) {
        let mut pass_encoder = encoder.new_pass(&self.name, &self.render_targets, registry);
        self.inner.execute(&registry, &mut pass_encoder);
    }
}

pub struct RenderPassBuilder {
    name: String,
    shader_name: String,
    inputs: PassParameter,
    outputs: PassParameter,

    inner: Option<Arc<dyn RenderPassExecutor>>,

    render_targets: PassRenderTargets,
    bind_default: bool,
}

impl RenderPassBuilder {
    pub fn new<S: Into<String>>(name: S) -> Self {
        Self {
            name: name.into(),
            shader_name: String::new(),
            inputs: PassParameter::default(),
            outputs: PassParameter::default(),
            inner: None,
            render_targets: PassRenderTargets::default(),
            bind_default: false,
        }
    }

    pub fn read_texture(&mut self, input: ResourceId) {
        self.inputs.textures.push((input, ResourceOp::TextureRead))
    }

    pub fn write_texture(&mut self, output: ResourceId) {
        self.outputs
            .textures
            .push((output, ResourceOp::TextureWrite))
    }

    pub fn read_buffer(&mut self, input: ResourceId) {
        self.inputs.buffers.push((input, ResourceOp::BufferRead))
    }

    pub fn read_write_texture(&mut self, t: ResourceId) {
        self.inputs
            .textures
            .push((t, ResourceOp::TextureReadAndWrite));
        self.outputs
            .textures
            .push((t, ResourceOp::TextureReadAndWrite));
    }

    pub fn set_shader_name<S: Into<String>>(&mut self, shader_name: S) {
        self.shader_name = shader_name.into();
    }

    pub fn async_execute(&mut self, exec: Arc<dyn RenderPassExecutor>) {
        self.inner = Some(exec);
    }

    pub fn bind_default_render_target(&mut self) {
        self.bind_default = true;
    }

    pub fn bind_render_targets(&mut self, render_targets: PassRenderTargets) {
        self.render_targets = render_targets;
        self.bind_default = false;
    }

    pub(crate) fn build(self) -> RenderPass {
        RenderPass {
            inner: self.inner.unwrap(),
            name: self.name,
            shader_name: self.shader_name,
            inputs: self.inputs,
            outputs: self.outputs,
            render_targets: self.render_targets,
            bind_default: self.bind_default,
        }
    }
}
