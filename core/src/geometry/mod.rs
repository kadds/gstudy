use crate::{scene::Transform, types::*, util::any_as_u8_slice_array};
use std::{
    any::Any,
    collections::{BTreeMap, HashMap},
    fmt::Debug,
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct IntersectResult {
    pos: Vec3f,
    color: Vec4f,
    normal: Vec3f,
    reflection_ray: Ray,
    refraction_ray: Ray,
}

#[repr(u8)]
#[derive(Debug, Hash, Eq, Ord, PartialEq, PartialOrd, Clone, Copy)]
pub enum MeshCoordType {
    Color,
    Pos2,
    TexCoord,
    TexNormal,
    TexBump,
    TexCube,
    ColorUint,
}

// #[derive(Debug)]
// pub enum MeshCoordValue {
//     Color(Vec<Vec4f>),
//     TexCoord(Vec<Vec2f>),
//     Pos2(Vec<Vec2f>),
//     ColorUint(Vecuint32),
// }

impl MeshCoordType {
    pub fn size_alignment(&self) -> (u64, u64) {
        match self {
            MeshCoordType::Color => (16, 16),
            MeshCoordType::Pos2 => (8, 8),
            MeshCoordType::TexCoord => (8, 8),
            MeshCoordType::TexNormal => (8, 8),
            MeshCoordType::TexBump => (8, 8),
            MeshCoordType::TexCube => (8, 8),
            MeshCoordType::ColorUint => (4, 4),
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

#[derive(Debug)]
struct BytesOffset {
    offset: u32,
    len: u32,
}

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vec3f>,
    pub indices: Vec<u32>,
    pub clip: Option<Rectu>,

    mesh_coord: BTreeMap<MeshCoordType, BytesOffset>,

    pub row_strip_size: u32,
    pub row_data_size: u32,

    pub coord_props: Vec<u8>,
    pub vertex_count: u32,
    pub has_position: bool,
}

impl std::fmt::Debug for Mesh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mesh")
            .field("vertices", &self.vertices)
            .field("indices", &self.indices)
            .field("clip", &self.clip)
            .finish()
    }
}

impl Mesh {
    pub fn new(has_position: bool, props: &[MeshCoordType]) -> Self {
        let mut mesh_coord = BTreeMap::new();
        let mut offset = 0;
        let mut max_alignment = 0;

        for prop in props {
            let (size, alignment) = prop.size_alignment();
            let rest = offset % alignment;
            if rest < size {
                if rest != 0 {
                    offset += alignment - rest;
                }
            }
            max_alignment = max_alignment.max(alignment);
            mesh_coord.insert(
                *prop,
                BytesOffset {
                    offset: offset as u32,
                    len: size as u32,
                },
            );
            offset += size;
        }
        if max_alignment == 0 {
            Self {
                row_data_size: 0,
                row_strip_size: 0,
                mesh_coord,
                has_position,
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
                row_data_size,
                row_strip_size,
                mesh_coord,
                has_position,
                ..Default::default()
            }
        }
    }

    pub fn clip(&self) -> Option<Rectu> {
        self.clip
    }

    pub fn indices(&self) -> &[u8] {
        any_as_u8_slice_array(&self.indices)
    }

    pub fn vertices(&self) -> &[u8] {
        any_as_u8_slice_array(&self.vertices)
    }

    pub fn vertices_props(&self) -> &[u8] {
        &self.coord_props
    }

    pub fn index_count(&self) -> u32 {
        self.indices.len() as u32
    }

    pub fn apply(&mut self, transform: &Transform) {
        let mut tmp = Vec::new();
        core::mem::swap(&mut tmp, &mut self.vertices);

        let vertices = transform.apply_batch(tmp.into_iter()).collect();
        self.vertices = vertices;
    }
}

pub struct MeshDataBuilder {
    mesh: Mesh,
    props_write_map: HashMap<MeshCoordType, u64>,
    max_count: u64,
    max_props_count: u64,
}

impl MeshDataBuilder {
    pub fn add_vertex_position(&mut self, vertex: Vec3f, prop: &[u8]) {
        self.mesh.vertices.push(vertex);
        self.add_vertex(prop)
    }

    pub fn add_indices(&mut self, indices: &[u32]) {
        self.mesh.indices.extend(indices)
    }

    pub fn add_vertex(&mut self, prop: &[u8]) {
        assert_eq!(self.mesh.row_data_size, prop.len() as u32);

        self.mesh.coord_props.extend_from_slice(prop);
        let empty: [u64; 4] = [0, 0, 0, 0];
        let slice = any_as_u8_slice_array(&empty);

        self.mesh.coord_props.extend_from_slice(
            &slice[..(self.mesh.row_strip_size - self.mesh.row_data_size) as usize],
        );
        self.max_count += 1;
    }

    pub fn add_position(&mut self, pos: &[Vec3f]) {
        self.mesh.vertices.extend_from_slice(pos);
        self.max_count += pos.len() as u64;
    }

    pub fn add_vertices_prop(&mut self, prop_name: MeshCoordType, prop: &[u8], strip: u32) {
        let write_count = prop.len() as u64 / prop_name.size_alignment().0;
        let prev_write_count = *self.props_write_map.get(&prop_name).unwrap();

        let result_count = write_count + prev_write_count;
        let result_max = self.max_props_count.max(result_count);

        let target_size = (result_max * self.mesh.row_strip_size as u64) as usize;
        if self.mesh.coord_props.len() < target_size {
            self.mesh.coord_props.resize(target_size, 0);
        }

        let o = self.mesh.mesh_coord.get(&prop_name).unwrap();

        let mut prop_offset = 0;
        let mut beg_offset =
            prev_write_count as usize * self.mesh.row_strip_size as usize + o.offset as usize;

        while prop_offset < prop.len() {
            let end = strip as usize + prop_offset;
            let src_slice = &prop[prop_offset..end];
            let end_offset = beg_offset + o.len as usize;
            let dst_slice = &mut self.mesh.coord_props[beg_offset..end_offset];

            dst_slice.copy_from_slice(src_slice);

            beg_offset += self.mesh.row_strip_size as usize;
            prop_offset += strip as usize;
        }
        *self.props_write_map.get_mut(&prop_name).unwrap() = result_count;

        self.max_props_count = result_max;
    }

    pub fn set_raw_props(&mut self, data: &[u8]) {
        self.mesh.coord_props.extend_from_slice(data)
    }

    pub fn set_clip(&mut self, clip: Rectu) {
        self.mesh.clip = Some(clip);
    }

    pub fn build(self) -> Mesh {
        if self.mesh.has_position && !self.props_write_map.is_empty() {
            assert_eq!(self.max_count, self.max_props_count);
        }
        self.mesh
    }
}

pub struct MeshBuilder {
    props: Vec<MeshCoordType>,
    no_position: bool,
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self {
            props: Vec::new(),
            no_position: false,
        }
    }

    pub fn set_no_position(&mut self) {
        self.no_position = true;
    }

    pub fn add_props(&mut self, props: MeshCoordType) {
        self.props.push(props);
    }

    pub fn finish_props(&mut self) -> MeshDataBuilder {
        let mut props_write_map = HashMap::new();
        for prop in &self.props {
            props_write_map.insert(*prop, 0);
        }
        self.props.sort();

        MeshDataBuilder {
            mesh: Mesh::new(!self.no_position, &self.props),
            props_write_map,
            max_count: 0,
            max_props_count: 0,
        }
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
}

#[derive(Debug)]
pub struct StaticGeometry {
    mesh: Arc<Mesh>,
    transform: Transform,
    attributes: HashMap<Attribute, Arc<dyn Any + Send + Sync>>,
}

impl StaticGeometry {
    pub fn new(mesh: Arc<Mesh>) -> Self {
        Self {
            mesh,
            transform: Transform::default(),
            attributes: HashMap::new(),
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
}

#[derive(Debug)]
struct DirtyMesh {
    dirty_flag: bool,
    version: u64,
    mesh: Option<Arc<Mesh>>,
}

impl Default for DirtyMesh {
    fn default() -> Self {
        Self {
            dirty_flag: true,
            version: 0,
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
