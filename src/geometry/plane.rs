use std::sync::Arc;
use std::sync::Mutex;

use super::BasicGeometry;
use super::DirtyMesh;
use super::GeometryMeshGenerator;
use super::MeshCoordType;
use super::{Geometry, Mesh, Ray};
use crate::render::Transform;
use crate::types::*;
// normal (0, 1, 0)
#[derive(Debug)]
pub struct PlaneMesh {
    color: Vec4f,
}

impl PlaneMesh {
    pub fn new() -> Self {
        Self {
            color: Vec4f::new(1f32, 1f32, 1f32, 1f32),
        }
    }
}
impl GeometryMeshGenerator for PlaneMesh {
    fn build_mesh(&self) -> Option<Mesh> {
        let mut mesh = Mesh::new();
        mesh.add_indices(&vec![0, 1, 2, 2, 3, 0]);

        mesh.add_vertex(Vec3f::new(-1f32, 0f32, 1f32));
        mesh.add_vertex(Vec3f::new(1f32, 0f32, 1f32));
        mesh.add_vertex(Vec3f::new(1f32, 0f32, -1f32));
        mesh.add_vertex(Vec3f::new(-1f32, 0f32, -1f32));

        mesh.set_coord_vec4f(
            MeshCoordType::Color,
            vec![self.color, self.color, self.color, self.color],
        );

        Some(mesh)
    }
}

pub type Plane = BasicGeometry<PlaneMesh>;
