use std::sync::Arc;
use std::sync::Mutex;

use super::{Geometry, Mesh, MeshTexture, Ray};
use crate::render::Transform;
use crate::types::*;

#[derive(Debug)]
struct Inner {
    dirty_flag: bool,
    mesh: Option<Arc<Mesh>>,
}

// normal (0, 1, 0)
#[derive(Debug)]
pub struct Plane {
    size: Vec2f,
    inner: Mutex<Inner>,
    transform: Transform,
}

impl Plane {
    pub fn new(size: Vec2f) -> Self {
        Self {
            size,
            inner: Inner {
                dirty_flag: true,
                mesh: None,
            }
            .into(),
            transform: Transform::default(),
        }
    }

    fn build_mesh(&self) -> Arc<Mesh> {
        let mut vertices = Vec::new();
        let indices = vec![0, 1, 2, 2, 3, 0];
        vertices.push(Vec3f::new(-self.size.x, 0f32, self.size.y));
        vertices.push(Vec3f::new(self.size.x, 0f32, self.size.y));
        vertices.push(Vec3f::new(self.size.x, 0f32, -self.size.y));
        vertices.push(Vec3f::new(-self.size.x, 0f32, -self.size.y));

        let mesh = Mesh { vertices, indices };

        Arc::new(mesh)
    }
}

impl Geometry for Plane {
    fn mesh_texture(&self) -> MeshTexture {
        {
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
    }

    fn intersect(&self, ray: Ray) -> super::IntersectResult {
        todo!()
    }
}
