use std::sync::{Arc, Mutex};

use crate::{render::Transform, types::Vec3f};

use super::{DirtyMesh, Geometry, Mesh, MeshTexture};

#[derive(Debug)]
pub struct Sphere {
    segments: u32,
    segments_vertical: u32,

    inner: Mutex<DirtyMesh>,
    transform: Transform,
}

impl Sphere {
    pub fn new(segments: u32, segments_vertical: u32) -> Self {
        Self {
            segments,
            segments_vertical,

            inner: Mutex::new(DirtyMesh::default()),
            transform: Transform::default(),
        }
    }

    fn build_mesh(&self) -> Arc<Mesh> {
        let mut mesh = Mesh::new();

        // mesh.add_vertex(Vec3f::new(0f32, 1f32, 0f32));

        mesh.apply(&self.transform);
        Arc::new(mesh)
    }

    pub fn set_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
}

impl Geometry for Sphere {
    fn mesh_texture(&self) -> MeshTexture {
        let mut inner = self.inner.lock().unwrap();
        if inner.dirty_flag {
            inner.mesh = Some(self.build_mesh());
            inner.dirty_flag = false;
        }
        MeshTexture {
            mesh: inner.mesh.as_ref().unwrap().clone(),
            optional: None,
        }
    }

    fn intersect(&self, ray: super::Ray) -> super::IntersectResult {
        todo!()
    }
}
