use std::sync::Arc;
use std::sync::Mutex;

use super::DirtyMesh;
use super::{Geometry, Mesh, MeshTexture, Ray};
use crate::render::Transform;
use crate::types::*;
// normal (0, 1, 0)
#[derive(Debug)]
pub struct Plane {
    inner: Mutex<DirtyMesh>,
    transform: Transform,
}

impl Plane {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(DirtyMesh::default()),
            transform: Transform::default(),
        }
    }

    fn build_mesh(&self) -> Arc<Mesh> {
        let mut mesh = Mesh::new();
        mesh.add_indices(&vec![0, 1, 2, 2, 3, 0]);

        mesh.add_vertex(Vec3f::new(-1f32, 0f32, 1f32));
        mesh.add_vertex(Vec3f::new(1f32, 0f32, 1f32));
        mesh.add_vertex(Vec3f::new(1f32, 0f32, -1f32));
        mesh.add_vertex(Vec3f::new(-1f32, 0f32, -1f32));

        mesh.apply(&self.transform);

        Arc::new(mesh)
    }

    pub fn set_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self.inner.lock().unwrap().dirty_flag = true;
        self
    }
}

impl Geometry for Plane {
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

    fn intersect(&self, ray: Ray) -> super::IntersectResult {
        todo!()
    }
}
