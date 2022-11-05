use std::sync::Arc;
use std::sync::Mutex;

use super::BasicGeometry;
use super::DirtyMesh;
use super::GeometryMeshGenerator;
use super::{Geometry, Mesh, Ray};
use crate::render::Transform;
use crate::types::*;
// normal (0, 1, 0)
#[derive(Debug)]
pub struct PlaneMesh {
}


impl PlaneMesh {
    pub fn new() -> Self {
        Self {
        }
    }

}
impl GeometryMeshGenerator for PlaneMesh {
    fn build_mesh(&self) -> Mesh {
        let mut mesh = Mesh::new();
        mesh.add_indices(&vec![0, 1, 2, 2, 3, 0]);

        mesh.add_vertex(Vec3f::new(-1f32, 0f32, 1f32));
        mesh.add_vertex(Vec3f::new(1f32, 0f32, 1f32));
        mesh.add_vertex(Vec3f::new(1f32, 0f32, -1f32));
        mesh.add_vertex(Vec3f::new(-1f32, 0f32, -1f32));

        mesh
    }
}

pub type Plane = BasicGeometry<PlaneMesh>;
