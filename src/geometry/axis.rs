use std::sync::{Arc, Mutex};

use crate::{render::Transform, types::Vec3f};

use super::{DirtyMesh, Geometry, Mesh, Topology};

#[derive(Debug)]
pub struct Axis {
    inner: Mutex<DirtyMesh>,
    transform: Transform,
}

impl Axis {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(DirtyMesh::default()),
            transform: Transform::default(),
        }
    }

    fn build_mesh(&self) -> Arc<Mesh> {
        let mut mesh = Mesh::new();
        mesh.add_vertex(Vec3f::new(0f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(1f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(0f32, 1f32, 0f32));
        mesh.add_vertex(Vec3f::new(0f32, 0f32, 1f32));

        mesh.add_indices(&[0, 1, 0, 2, 0, 3]);
        mesh.set_topology(Topology::Line);

        mesh.apply(&self.transform);

        Arc::new(mesh)
    }

    pub fn set_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
}

impl Geometry for Axis {
    fn mesh_texture(&self) -> super::MeshTexture {
        let mut inner = self.inner.lock().unwrap();
        if inner.dirty_flag {
            inner.mesh = Some(self.build_mesh());
            inner.dirty_flag = false;
        }
        super::MeshTexture {
            mesh: inner.mesh.as_ref().unwrap().clone(),
            optional: None,
        }
    }

    fn intersect(&self, ray: super::Ray) -> super::IntersectResult {
        todo!()
    }
}
