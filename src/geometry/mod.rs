use crate::{render::Transform, types::*};
use std::{fmt::Debug, sync::Arc};

#[derive(Debug)]
pub struct Mesh {
    pub vertices: Vec<Vec3f>,
    pub indices: Vec<u32>,
    pub topology: Topology,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            topology: Topology::Triangle,
        }
    }

    #[inline]
    pub fn set_topology(&mut self, topo: Topology) {
        self.topology = topo;
    }

    #[inline]
    pub fn add_vertex(&mut self, vertex: Vec3f) {
        self.vertices.push(vertex);
    }

    pub fn add_triangle(&mut self, v0: Vec3f, v1: Vec3f, v2: Vec3f) {
        self.vertices.push(v0);
        self.vertices.push(v1);
        self.vertices.push(v2);
    }

    #[inline]
    pub fn add_index(&mut self, index: u32) {
        self.indices.push(index)
    }

    pub fn add_indices(&mut self, indices: &[u32]) {
        self.indices.extend(indices)
    }

    pub fn apply(&mut self, transform: &Transform) {
        let mut tmp = Vec::new();
        core::mem::swap(&mut tmp, &mut self.vertices);

        let vertices = transform.apply_batch(tmp.into_iter()).collect();
        self.vertices = vertices;
    }
}

#[derive(Debug)]
pub enum Topology {
    Point,
    Line,
    Triangle,
}

#[derive(Debug)]
pub struct OptionalMesh {
    pub vertices_color: Vec<Vec4f>,
    pub vertices_texcoord: Vec<Vec2f>,
}

#[derive(Debug)]
pub struct IntersectResult {
    pos: Vec3f,
    color: Vec4f,
    normal: Vec3f,
    reflection_ray: Ray,
    refraction_ray: Ray,
}

#[derive(Debug)]
pub struct MeshTexture {
    pub mesh: Arc<Mesh>,
    pub optional: Option<Arc<OptionalMesh>>,
}

pub trait Geometry: Send + Sync + Debug {
    fn mesh_texture(&self) -> MeshTexture;
    fn intersect(&self, ray: Ray) -> IntersectResult;
}

pub mod axis;
pub mod plane;
pub mod sphere;

#[derive(Debug)]
struct DirtyMesh {
    dirty_flag: bool,
    mesh: Option<Arc<Mesh>>,
}

impl Default for DirtyMesh {
    fn default() -> Self {
        Self {
            dirty_flag: true,
            mesh: Default::default(),
        }
    }
}

#[derive(Debug)]
pub struct Ray {
    pos: Vec3f,
    dir: Vec3f,
    color: Vec4f,
}

impl Ray {
    pub fn new(pos: Vec3f, dir: Vec3f, color: Vec4f) -> Self {
        Self { pos, dir, color }
    }
}
