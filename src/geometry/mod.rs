use crate::{render::Transform, types::*};
use std::{
    any::Any,
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub enum Topology {
    Point,
    Line,
    Triangle,
}

impl Default for Topology {
    fn default() -> Self {
        Self::Triangle
    }
}

#[derive(Debug)]
pub struct IntersectResult {
    pos: Vec3f,
    color: Vec4f,
    normal: Vec3f,
    reflection_ray: Ray,
    refraction_ray: Ray,
}

#[derive(Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3f>,
    pub indices: Vec<u32>,
    pub topology: Topology,

    pub vertices_color: Option<Vec<Vec4f>>,
    pub vertices_texcoord: Option<Vec<Vec2f>>,
    pub vertices_normal_coord: Option<Vec<Vec2f>>,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            ..Default::default()
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

pub trait Geometry: Send + Sync + Debug {
    fn mesh(&self) -> Arc<Mesh>;
    fn intersect(&self, ray: Ray) -> IntersectResult;
    fn attribute(&self, attribute: &Attribute) -> Option<Arc<dyn Any + Send + Sync>>;
    fn set_attribute(&mut self, attribute: Attribute, value: Arc<dyn Any + Send + Sync>);
}

#[derive(Debug)]
pub struct StaticGeometry {
    mesh: Arc<Mesh>,
    attributes: HashMap<Attribute, Arc<dyn Any + Send + Sync>>,
}

impl StaticGeometry {
    pub fn new(mesh: Arc<Mesh>) -> Self {
        Self {
            mesh,
            attributes: HashMap::new(),
        }
    }
}

impl Geometry for StaticGeometry {
    fn mesh(&self) -> Arc<Mesh> {
        self.mesh.clone()
    }

    fn intersect(&self, ray: Ray) -> IntersectResult {
        todo!()
    }

    fn attribute(&self, attribute: &Attribute) -> Option<Arc<dyn Any + Send + Sync>> {
        self.attributes.get(attribute).cloned()
    }

    fn set_attribute(&mut self, attribute: Attribute, value: Arc<dyn Any + Send + Sync>) {
        self.attributes.insert(attribute, value);
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub enum Attribute {
    ConstantColor,
    UV,
    Name(String),
    Index(usize),
}

pub trait GeometryMeshGenerator: Send + Sync + Debug {
    fn build_mesh(&self) -> Mesh;
}

#[derive(Debug)]
pub struct BasicGeometry<G>
where
    G: GeometryMeshGenerator,
{
    inner: Mutex<DirtyMesh>,
    transform: Transform,
    attributes: HashMap<Attribute, Arc<dyn Any + Send + Sync>>,
    g: G,
}

impl<G> BasicGeometry<G>
where
    G: GeometryMeshGenerator,
{
    pub fn new(g: G) -> Self {
        Self {
            inner: Mutex::new(DirtyMesh::default()),
            transform: Transform::default(),
            attributes: HashMap::new(),
            g,
        }
    }

    pub fn build_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self.inner.lock().unwrap().dirty_flag = true;
        self
    }
}

impl<G> Geometry for BasicGeometry<G>
where
    G: GeometryMeshGenerator,
{
    fn mesh(&self) -> Arc<Mesh> {
        let mut inner = self.inner.lock().unwrap();
        if inner.dirty_flag {
            let mut mesh = self.g.build_mesh();
            mesh.apply(&self.transform);

            inner.mesh = Some(Arc::new(mesh));
            inner.dirty_flag = false;
        }
        inner.mesh.as_ref().unwrap().clone()
    }

    fn intersect(&self, ray: Ray) -> IntersectResult {
        todo!()
    }

    fn attribute(&self, attribute: &Attribute) -> Option<Arc<dyn Any + Send + Sync>> {
        self.attributes.get(attribute).cloned()
    }

    fn set_attribute(&mut self, attribute: Attribute, value: Arc<dyn Any + Send + Sync>) {
        self.attributes.insert(attribute, value);
    }
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
