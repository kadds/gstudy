use std::fmt::Debug;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use super::{
    backends::wgpu_backend::PipelinePass,
    ps::{PipelineStateBuilder, PipelineStateObject},
};

#[derive(Debug)]
pub struct RContext {
    last_pso_id: AtomicU64,
    last_texture_id: AtomicU64,
    last_object_id: AtomicU64,
    last_material_id: AtomicU64,

    inner: Box<dyn RContextImpl>,
}

impl RContext {
    pub fn new(inner: Box<dyn RContextImpl>) -> RContextRef {
        Arc::new(Self {
            last_pso_id: AtomicU64::new(0),
            last_texture_id: AtomicU64::new(0),
            last_object_id: AtomicU64::new(0),
            last_material_id: AtomicU64::new(0),
            inner,
        })
    }

    pub fn alloc_pso(&self) -> u64 {
        self.last_pso_id.fetch_add(1, Ordering::SeqCst)
    }
    pub fn alloc_object_id(&self) -> u64 {
        self.last_object_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn alloc_material_id(&self) -> u64 {
        self.last_material_id.fetch_add(1, Ordering::SeqCst)
    }

    pub fn inner(&self) -> &dyn RContextImpl {
        self.inner.as_ref()
    }
}

pub trait RContextImpl: Debug + Send + Sync {
    fn map_pso(&self, pso: u64, value: Option<PipelinePass>);
    fn get_pso(&self, pso: u64) -> Arc<PipelinePass>;
}

pub type RContextRef = Arc<RContext>;
