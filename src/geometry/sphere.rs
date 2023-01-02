use std::sync::{Arc, Mutex};

use crate::{render::Transform, types::Vec3f};

use super::{BasicGeometry, DirtyMesh, Geometry, GeometryMeshGenerator, Mesh};

#[derive(Debug)]
pub struct SphereMesh {
    segments: u32,
    segments_vertical: u32,
}

impl SphereMesh {
    pub fn new(segments: u32, segments_vertical: u32) -> Self {
        Self {
            segments,
            segments_vertical,
        }
    }
}

impl GeometryMeshGenerator for SphereMesh {
    fn build_mesh(&self) -> Option<Mesh> {
        let mut mesh = Mesh::new();

        // mesh.add_vertex(Vec3f::new(0f32, 1f32, 0f32));

        Some(mesh)
    }
}

pub type Sphere = BasicGeometry<SphereMesh>;
