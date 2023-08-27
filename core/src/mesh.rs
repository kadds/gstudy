use crate::{scene::Transform, types::*, util::any_as_u8_slice_array};
use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    sync::{Arc, Mutex},
};

use self::intersect::{IntersectResult, Ray};

pub mod builder;
pub mod intersect;

#[repr(u8)]
#[derive(Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Clone, Copy)]
pub enum MeshPropertyType {
    Color,
    Pos2,
    TexCoord,
    TexNormal,
    TexBump,
    TexCube,
    ColorUint,
}

impl MeshPropertyType {
    pub fn size_alignment(&self) -> (u32, u32) {
        match self {
            MeshPropertyType::Color => (16, 16),
            MeshPropertyType::Pos2 => (8, 8),
            MeshPropertyType::TexCoord => (8, 8),
            MeshPropertyType::TexNormal => (8, 8),
            MeshPropertyType::TexBump => (8, 8),
            MeshPropertyType::TexCube => (8, 8),
            MeshPropertyType::ColorUint => (4, 4),
        }
    }
}

pub type MeshTransformer = Box<dyn FnMut(Vec<u8>, &Transform) -> Vec<u8> + Send + Sync>;

fn default_transformer(data: Vec<u8>, t: &Transform) -> Vec<u8> {
    data
}

pub fn load_default_transformer() -> MeshTransformer {
    Box::new(default_transformer)
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct FieldOffset {
    offset: u32,
    len: u32,
}

#[derive(Debug, Default)]
pub(crate) enum Indices {
    #[default]
    Unknown,
    None,
    U32(Vec<u32>),
    U16(Vec<u16>),
}

#[derive(Debug, Default)]
pub(crate) enum PositionVertices {
    #[default]
    Unknown,
    None,
    F2(Vec<Vec2f>),
    F3(Vec<Vec3f>),
    F4(Vec<Vec4f>),
}

#[derive(Default)]
pub struct Mesh {
    pub(crate) position_vertices: PositionVertices,
    pub(crate) indices: Indices,
    pub(crate) clip: Option<Rectu>,

    pub(crate) properties_offset: BTreeMap<MeshPropertyType, FieldOffset>,

    pub(crate) row_strip_size: u32,
    pub(crate) row_size: u32,

    pub(crate) properties: Vec<u8>,
    pub(crate) vertex_count: usize,
}

impl std::fmt::Debug for Mesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mesh")
            // .field("position_vertices", self.position_vertices.len())
            // .field("vertices", self.properties.len())
            .field("indices", &self.indices)
            .field("clip", &self.clip)
            .finish()
    }
}

impl Mesh {
    pub fn new<'a, I: Iterator<Item = &'a MeshPropertyType>>(properties: I) -> Self {
        let mut properties_offset = BTreeMap::new();
        let mut offset = 0;
        let mut max_alignment = 0;

        for prop in properties {
            let (size, alignment) = prop.size_alignment();
            let rest = offset % alignment;
            if rest < size {
                if rest != 0 {
                    offset += alignment - rest;
                }
            }
            max_alignment = max_alignment.max(alignment);
            properties_offset.insert(
                *prop,
                FieldOffset {
                    offset: offset as u32,
                    len: size as u32,
                },
            );
            offset += size;
        }
        if max_alignment == 0 {
            Self {
                row_size: 0,
                row_strip_size: 0,
                properties_offset,
                ..Default::default()
            }
        } else {
            let row_data_size = offset as u32;

            let rest = offset % max_alignment;
            if rest != 0 {
                // offset += max_alignment - rest;
            }
            let row_strip_size = offset as u32;

            Self {
                row_size: row_data_size,
                row_strip_size,
                properties_offset,
                ..Default::default()
            }
        }
    }

    pub fn row_strip_size(&self) -> u32 {
        self.row_strip_size
    }

    pub fn aabb(&self) -> Option<BoundBox> {
        // if self.has_position {
        //     let mut aabb = BoundBox::default();
        //     for a in &self.vertices {
        //         aabb = &aabb + a;
        //     }
        //     return Some(aabb);
        // }
        None
    }

    pub fn clip(&self) -> Option<Rectu> {
        self.clip
    }

    pub fn indices_view(&self) -> Option<&[u8]> {
        match &self.indices {
            Indices::U32(d) => Some(any_as_u8_slice_array(&d)),
            Indices::U16(d) => Some(any_as_u8_slice_array(&d)),
            _ => None,
        }
    }

    pub fn indices_is_u32(&self) -> Option<bool> {
        match &self.indices {
            Indices::U32(d) => Some(true),
            Indices::U16(d) => Some(false),
            _ => None,
        }
    }

    pub fn vertices_view(&self) -> Option<&[u8]> {
        match &self.position_vertices {
            PositionVertices::F2(d) => Some(any_as_u8_slice_array(&d)),
            PositionVertices::F3(d) => Some(any_as_u8_slice_array(&d)),
            PositionVertices::F4(d) => Some(any_as_u8_slice_array(&d)),
            _ => None,
        }
    }

    pub fn properties_view(&self) -> &[u8] {
        &self.properties
    }

    pub fn index_count(&self) -> Option<u32> {
        match &self.indices {
            Indices::U32(i) => Some(i.len() as u32),
            Indices::U16(i) => Some(i.len() as u32),
            _ => None,
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    pub fn apply(&mut self, transform: &Transform) {
        // let mut tmp = Vec::new();
        // core::mem::swap(&mut tmp, &mut self.vertices);

        // let vertices = transform.apply_batch(tmp.into_iter()).collect();
        // self.vertices = vertices;
    }
}

pub trait Geometry: Send + Sync + Debug {
    fn mesh(&self) -> Arc<Mesh>;
    fn intersect(&self, ray: Ray) -> IntersectResult;
    fn attribute(&self, attribute: &Attribute) -> Option<Arc<dyn Any + Send + Sync>>;
    fn set_attribute(&mut self, attribute: Attribute, value: Arc<dyn Any + Send + Sync>);
    fn is_static(&self) -> bool;
    fn mesh_version(&self) -> u64;

    fn transform(&self) -> &Transform;
    fn aabb(&self) -> Option<BoundBox>;
}

#[derive(Debug)]
pub struct StaticGeometry {
    mesh: Arc<Mesh>,
    transform: Transform,
    attributes: HashMap<Attribute, Arc<dyn Any + Send + Sync>>,
    aabb: Option<BoundBox>,
}

impl StaticGeometry {
    pub fn new(mesh: Arc<Mesh>) -> Self {
        let aabb = mesh.aabb();
        Self {
            mesh,
            transform: Transform::default(),
            attributes: HashMap::new(),
            aabb,
        }
    }
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
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

    fn is_static(&self) -> bool {
        true
    }

    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn mesh_version(&self) -> u64 {
        0
    }

    fn aabb(&self) -> Option<BoundBox> {
        self.aabb.clone()
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
    fn build_mesh(&self) -> Option<Mesh>;
}

#[derive(Debug)]
pub struct BasicGeometry<G>
where
    G: GeometryMeshGenerator,
{
    inner: Mutex<DirtyMesh>,
    transform: Transform,
    attributes: HashMap<Attribute, Arc<dyn Any + Send + Sync>>,
    is_static: bool,
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
            is_static: false,
            g,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.inner.lock().unwrap().dirty_flag = true;
    }

    pub fn build_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self.inner.lock().unwrap().dirty_flag = true;
        self
    }

    pub fn with_static(mut self, is_static: bool) -> Self {
        self.is_static = is_static;
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
            let mut mesh = match self.g.build_mesh() {
                Some(v) => v,
                None => {
                    return inner.mesh.as_ref().unwrap().clone();
                }
            };
            mesh.apply(&self.transform);
            let aabb = mesh.aabb();

            inner.mesh = Some(Arc::new(mesh));
            inner.dirty_flag = false;
            inner.aabb = aabb;
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

    fn is_static(&self) -> bool {
        self.is_static
    }

    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn mesh_version(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.version
    }

    fn aabb(&self) -> Option<BoundBox> {
        let mut inner = self.inner.lock().unwrap();
        inner.aabb.clone()
    }
}

#[derive(Debug)]
struct DirtyMesh {
    dirty_flag: bool,
    version: u64,
    mesh: Option<Arc<Mesh>>,
    aabb: Option<BoundBox>,
}

impl Default for DirtyMesh {
    fn default() -> Self {
        Self {
            dirty_flag: true,
            version: 0,
            mesh: Default::default(),
            aabb: None,
        }
    }
}
