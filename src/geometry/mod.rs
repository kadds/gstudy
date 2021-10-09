use crate::types::*;
use std::{fmt::Debug, sync::Arc};

#[derive(Debug)]
pub struct Mesh {
    pub vertices: Vec<Vec3f>,
    pub indices: Vec<u32>,
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

pub mod plane;

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
