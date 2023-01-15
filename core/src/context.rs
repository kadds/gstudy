use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use dashmap::DashMap;

#[derive(Debug)]
pub enum ResourceTy {
    // Pso(wgpu::RenderPipeline),
    Texture((wgpu::Texture, wgpu::TextureView)),
    SurfaceTexture((Arc<wgpu::SurfaceTexture>, wgpu::TextureView)),
}

#[derive(Debug)]
pub struct Resource {
    ty: ResourceTy,
    id: u64,
}

impl Resource {
    pub fn new(ty: ResourceTy, id: u64) -> Self {
        Self { ty, id }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
}

pub type ResourceRef = Arc<Resource>;

impl Resource {
    // pub fn pso_ref(&self) -> &wgpu::RenderPipeline {
    //     match self {
    //         Resource::Pso(p) => p,
    //         _ => panic!("resource type invalid"),
    //     }
    // }
    pub fn texture_view(&self) -> &wgpu::TextureView {
        match &self.ty {
            ResourceTy::Texture(p) => &p.1,
            ResourceTy::SurfaceTexture(t) => &t.1,
            _ => panic!("resource type invalid"),
        }
    }
    pub fn texture_ref(&self) -> &wgpu::Texture {
        match &self.ty {
            ResourceTy::Texture(t) => &t.0,
            ResourceTy::SurfaceTexture(t) => &t.0.texture,
            _ => panic!("resource type invalid"),
        }
    }
}

pub struct RContext {
    last_res_id: AtomicU64,
    last_object_id: AtomicU64,
    last_material_id: AtomicU64,
    last_camera_id: AtomicU64,

    res_map: DashMap<u64, Arc<Resource>>,
}

impl std::fmt::Debug for RContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RContext")
            .field("last_res_id", &self.last_res_id)
            .field("last_object_id", &self.last_object_id)
            .field("last_material_id", &self.last_material_id)
            .field("last_camera_id", &self.last_camera_id)
            .finish()
    }
}

impl RContext {
    pub fn new() -> RContextRef {
        Arc::new(Self {
            last_res_id: AtomicU64::new(1),
            last_object_id: AtomicU64::new(1),
            last_material_id: AtomicU64::new(1),
            last_camera_id: AtomicU64::new(1),
            res_map: DashMap::default(),
        })
    }

    pub(crate) fn alloc_object_id(&self) -> u64 {
        self.last_object_id.fetch_add(1, Ordering::SeqCst)
    }

    pub(crate) fn alloc_material_id(&self) -> u64 {
        self.last_material_id.fetch_add(1, Ordering::SeqCst)
    }
    pub(crate) fn alloc_camera_id(&self) -> u64 {
        self.last_camera_id.fetch_add(1, Ordering::SeqCst)
    }
    // pub(crate) fn register_pso(&self, pso: wgpu::RenderPipeline) -> PipelineStateObject {
    //     let id = self.last_res_id.fetch_add(1, Ordering::SeqCst);
    //     self.res_map
    //         .insert(id, (AtomicU64::new(1), Resource::Pso(pso)));
    //     PipelineStateObject::from_id(id, self.static_self())
    // }
    pub fn register_texture(&self, texture: wgpu::Texture) -> ResourceRef {
        let id = self.last_res_id.fetch_add(1, Ordering::SeqCst);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let res = Arc::new(Resource::new(ResourceTy::Texture((texture, view)), id));
        self.res_map.insert(id, res.clone());
        res
    }

    pub fn register_surface_texture(&self, texture: Arc<wgpu::SurfaceTexture>) -> ResourceRef {
        let id = self.last_res_id.fetch_add(1, Ordering::SeqCst);
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let res = Arc::new(Resource::new(
            ResourceTy::SurfaceTexture((texture, view)),
            id,
        ));

        self.res_map.insert(id, res.clone());
        res
    }

    pub fn deregister_by_id(&self, id: u64) {
        self.res_map.remove(&id);
    }
    pub fn deregister(&self, res: ResourceRef) {
        self.res_map.remove(&res.id());
    }
}

pub type RContextRef = Arc<RContext>;
