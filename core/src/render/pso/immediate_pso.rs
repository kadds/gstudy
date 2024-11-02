use std::sync::Arc;

use dashmap::DashMap;

use super::{load_pso, PipelineStateObject, PipelineStateObjectCache, RenderDescriptorObject};

pub struct ImmediatePipelineStateObjectCache {
    cache: DashMap<(u64, String), Arc<PipelineStateObject>>, // pass name -> object
}

impl PipelineStateObjectCache for ImmediatePipelineStateObjectCache {
    fn get(&self, id: u64, pass: Arc<tshader::Pass>) -> Arc<PipelineStateObject> {
        let t = self.cache.get(&(id, pass.name.clone()));
        if let Some(t) = t {
            return t.value().clone();
        }
        panic!();
    }

    fn load(&self, device: &wgpu::Device, id: u64, pass: Arc<tshader::Pass>, rdo: RenderDescriptorObject) {
        let pso = load_pso(device, pass.clone(), &rdo).unwrap();
        self.cache.insert((id, pass.name.clone()), pso);
    }
}

impl ImmediatePipelineStateObjectCache {
    pub fn new() -> Self {
        Self {
            cache: DashMap::new(),
        }
    }
}