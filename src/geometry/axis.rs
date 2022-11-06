use std::sync::{Arc, Mutex};

use crate::{
    render::Transform,
    types::{Vec3f, Vec4f},
};

use super::{BasicGeometry, DirtyMesh, Geometry, GeometryMeshGenerator, Mesh, Topology};

#[derive(Debug)]
pub struct AxisMesh {}

impl AxisMesh {
    pub fn new() -> Self {
        Self {}
    }
}

impl GeometryMeshGenerator for AxisMesh {
    fn build_mesh(&self) -> Mesh {
        let m = 1000000000f32;
        let mut mesh = Mesh::new();
        mesh.add_vertex(Vec3f::new(0f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(m, 0f32, 0f32));

        mesh.add_vertex(Vec3f::new(0f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(0f32, m, 0f32));

        mesh.add_vertex(Vec3f::new(0f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(0f32, 0f32, m));

        mesh.add_vertex(Vec3f::new(0f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(-m, 0f32, 0f32));

        mesh.add_vertex(Vec3f::new(0f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(0f32, -m, 0f32));

        mesh.add_vertex(Vec3f::new(0f32, 0f32, 0f32));
        mesh.add_vertex(Vec3f::new(0f32, 0f32, -m));

        mesh.add_indices(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
        mesh.set_topology(Topology::Line);

        let r = Vec4f::new(0.95f32, 0f32, 0f32, 1f32);
        let g = Vec4f::new(0f32, 0.95f32, 0f32, 1f32);
        let b = Vec4f::new(0f32, 0f32, 0.95f32, 1f32);

        let dr = Vec4f::new(0.5f32, 0f32, 0f32, 1f32);
        let dg = Vec4f::new(0f32, 0.5f32, 0f32, 1f32);
        let db = Vec4f::new(0f32, 0f32, 0.5f32, 1f32);

        mesh.vertices_color = Some(vec![r, r, g, g, b, b, dr, dr, dg, dg, db, db]);

        mesh
    }
}

pub type Axis = BasicGeometry<AxisMesh>;
