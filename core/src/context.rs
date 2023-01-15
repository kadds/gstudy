use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use dashmap::DashMap;

use crate::ds::{PipelineStateObject, Texture};

use super::backends::wgpu_backend::PipelinePass;

pub type ResourceId = u64;

pub enum Resource {
    Pso(PipelinePass),
    Texture((wgpu::Texture, wgpu::TextureView)),
    SurfaceTexture((Arc<wgpu::SurfaceTexture>, wgpu::TextureView)),
}

impl Resource {
    pub fn pso_ref(&self) -> &PipelinePass {
        match self {
            Resource::Pso(p) => p,
            _ => panic!("resource type invalid"),
        }
    }
    pub fn texture_view(&self) -> &wgpu::TextureView {
        match self {
            Resource::Texture(p) => &p.1,
            Resource::SurfaceTexture(t) => &t.1,
            _ => panic!("resource type invalid"),
        }
    }
    pub fn texture_ref(&self) -> &wgpu::Texture {
        match self {
            Resource::Texture(t) => &t.0,
            Resource::SurfaceTexture(t) => &t.0.texture,
            _ => panic!("resource type invalid"),
        }
    }
}

pub struct RContext {
    last_res_id: AtomicU64,
    last_object_id: AtomicU64,
    last_material_id: AtomicU64,
    last_camera_id: AtomicU64,

    res_map: DashMap<u64, (AtomicU64, Resource)>,
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

    pub(crate) fn add_ref(&self, id: ResourceId) {
        self.res_map.entry(id).and_modify(|v| {
            v.0.fetch_add(1, Ordering::SeqCst);
        });
    }

    fn static_self(&self) -> &'static Self {
        unsafe { std::mem::transmute(self) }
    }

    pub(crate) fn register_pso(&self, pso: PipelinePass) -> PipelineStateObject {
        let id = self.last_res_id.fetch_add(1, Ordering::SeqCst);
        self.res_map
            .insert(id, (AtomicU64::new(1), Resource::Pso(pso)));
        PipelineStateObject::from_id(id, self.static_self())
    }
    pub fn register_texture(&self, texture: wgpu::Texture) -> Texture {
        let id = self.last_res_id.fetch_add(1, Ordering::SeqCst);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.res_map
            .insert(id, (AtomicU64::new(1), Resource::Texture((texture, view))));
        Texture::from_id(id, self.static_self())
    }

    pub fn register_surface_texture(&self, texture: Arc<wgpu::SurfaceTexture>) -> Texture {
        let id = self.last_res_id.fetch_add(1, Ordering::SeqCst);
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.res_map.insert(
            id,
            (AtomicU64::new(1), Resource::SurfaceTexture((texture, view))),
        );
        Texture::from_id(id, self.static_self())
    }

    pub(crate) fn deref(&self, id: ResourceId) {
        let mut del = false;
        self.res_map.entry(id).and_modify(|v| {
            if v.0.fetch_sub(1, Ordering::SeqCst) == 1 {
                del = true;
            }
        });
        if del {
            self.res_map.remove(&id);
        }
    }

    pub(crate) fn get_resource<'a>(&'a self, id: ResourceId) -> &'a Resource {
        self.res_map
            .get(&id)
            .map(|v| unsafe { std::mem::transmute(&v.1) })
            .unwrap()
    }
}

pub type RContextRef = Arc<RContext>;
