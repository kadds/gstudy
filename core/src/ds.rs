use crate::{
    backends::wgpu_backend::{PipelinePass, WGPUResource},
    context::{RContext, RContextRef},
};

#[derive(Debug)]
pub struct DynamicResource {
    id: u64,
    context: &'static RContext,
}

impl DynamicResource {
    pub fn from_id(id: u64, context: &'static RContext) -> Self {
        Self { id, context }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
}

impl Clone for DynamicResource {
    fn clone(&self) -> Self {
        self.context.add_ref(self.id);
        Self {
            id: self.id.clone(),
            context: self.context.clone(),
        }
    }
}

impl Drop for DynamicResource {
    fn drop(&mut self) {
        self.context.deref(self.id);
    }
}

pub type Texture = DynamicResource;

impl Texture {
    pub(crate) fn internal_view(&self) -> &wgpu::TextureView {
        self.context.get_resource(self.id).texture_view()
    }
}

pub type PipelineStateObject = DynamicResource;
impl PipelineStateObject {
    pub fn internal(&self) -> &PipelinePass {
        self.context.get_resource(self.id).pso_ref()
    }
}
