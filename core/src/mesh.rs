use crate::{scene::Transform, types::*, util::any_as_u8_slice_array};
use std::{
    cell::RefCell,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use self::{
    builder::{
        FieldOffset, InstancePropertiesBuilder, InstancePropertyType, MeshPropertyType,
        PropertiesFrame, Property,
    },
    intersect::{IntersectResult, Ray},
};

pub mod builder;
pub mod intersect;

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

pub struct Mesh {
    pub(crate) position_vertices: PositionVertices,
    pub(crate) indices: Indices,
    pub(crate) clip: Option<Rectu>,

    pub(crate) vertex_count: u64,
    pub(crate) properties: PropertiesFrame<MeshPropertyType>,
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
    pub fn row_strip_size(&self) -> u32 {
        self.properties.row_strip_size
    }

    pub fn boundary(&self) -> Boundary {
        // if self.has_position {
        //     let mut aabb = BoundBox::default();
        //     for a in &self.vertices {
        //         aabb = &aabb + a;
        //     }
        //     return Some(aabb);
        // }
        Boundary::None
    }

    pub fn clip(&self) -> Option<Rectu> {
        self.clip
    }

    pub fn indices_view(&self) -> Option<&[u8]> {
        match &self.indices {
            Indices::U32(d) => Some(any_as_u8_slice_array(d)),
            Indices::U16(d) => Some(any_as_u8_slice_array(d)),
            _ => None,
        }
    }

    pub fn indices_is_u32(&self) -> Option<bool> {
        match &self.indices {
            Indices::U32(_) => Some(true),
            Indices::U16(_) => Some(false),
            _ => None,
        }
    }

    pub fn vertices_view(&self) -> Option<&[u8]> {
        match &self.position_vertices {
            PositionVertices::F2(d) => Some(any_as_u8_slice_array(d)),
            PositionVertices::F3(d) => Some(any_as_u8_slice_array(d)),
            PositionVertices::F4(d) => Some(any_as_u8_slice_array(d)),
            _ => None,
        }
    }

    pub fn properties_view(&self) -> &[u8] {
        self.properties.view()
    }

    pub fn index_count(&self) -> Option<u32> {
        match &self.indices {
            Indices::U32(i) => Some(i.len() as u32),
            Indices::U16(i) => Some(i.len() as u32),
            _ => None,
        }
    }

    pub fn vertex_count(&self) -> u64 {
        self.vertex_count
    }

    pub fn apply(&mut self, transform: &Transform) {
        // let mut tmp = Vec::new();
        // core::mem::swap(&mut tmp, &mut self.vertices);

        // let vertices = transform.apply_batch(tmp.into_iter()).collect();
        // self.vertices = vertices;
    }
}

pub struct GeometryInfo {
    pub is_static: bool,
    pub is_instance: bool,
}

pub trait Geometry: Send + Sync + Debug {
    fn mesh(&self) -> Arc<Mesh>;
    fn intersect(&self, ray: Ray) -> IntersectResult;
    fn info(&self) -> GeometryInfo;
    fn instance(&self) -> Option<&InstanceProperties>;

    fn transform(&self) -> &Transform;
    fn boundary(&self) -> &Boundary;
}

#[derive(Debug)]
pub enum TransformType {
    None,
    Mat4x4,
}

#[derive(Debug)]
pub struct InstanceProperties {
    pub data: Mutex<PropertiesFrame<InstancePropertyType>>,
    pub transform_type: TransformType,
}
#[derive(Debug)]
pub struct StaticGeometry {
    mesh: Arc<Mesh>,
    transform: Transform,
    boundary: Boundary,
    instance_data: Option<InstanceProperties>,
}

impl StaticGeometry {
    pub fn new(mesh: Arc<Mesh>) -> Self {
        let boundary = mesh.boundary();
        Self {
            mesh,
            transform: Transform::default(),
            boundary,
            instance_data: None,
        }
    }
    pub fn with_transform(mut self, transform: Transform) -> Self {
        self.transform = transform;
        self
    }
    pub fn with_instance(mut self, instance: InstanceProperties) -> Self {
        self.instance_data = Some(instance);
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

    fn info(&self) -> GeometryInfo {
        GeometryInfo {
            is_static: true,
            is_instance: false,
        }
    }

    fn transform(&self) -> &Transform {
        &self.transform
    }

    fn boundary(&self) -> &Boundary {
        &self.boundary
    }

    fn instance(&self) -> Option<&InstanceProperties> {
        self.instance_data.as_ref()
    }
}

pub trait GeometryMeshGenerator: Send + Sync + Debug {
    fn build_mesh(&self) -> Option<Mesh>;
}

// #[derive(Debug)]
// pub struct BasicGeometry<G>
// where
//     G: GeometryMeshGenerator,
// {
//     inner: Mutex<DirtyMesh>,
//     transform: Transform,
//     is_static: bool,
//     g: G,
// }

// impl<G> BasicGeometry<G>
// where
//     G: GeometryMeshGenerator,
// {
//     pub fn new(g: G) -> Self {
//         Self {
//             inner: Mutex::new(DirtyMesh::default()),
//             transform: Transform::default(),
//             is_static: false,
//             g,
//         }
//     }

//     pub fn mark_dirty(&mut self) {
//         self.inner.lock().unwrap().dirty_flag = true;
//     }

//     pub fn build_transform(mut self, transform: Transform) -> Self {
//         self.transform = transform;
//         self.inner.lock().unwrap().dirty_flag = true;
//         self
//     }

//     pub fn with_static(mut self, is_static: bool) -> Self {
//         self.is_static = is_static;
//         self
//     }
// }

// impl<G> Geometry for BasicGeometry<G>
// where
//     G: GeometryMeshGenerator,
// {
//     fn mesh(&self) -> Arc<Mesh> {
//         let mut inner = self.inner.lock().unwrap();
//         if inner.dirty_flag {
//             let mut mesh = match self.g.build_mesh() {
//                 Some(v) => v,
//                 None => {
//                     return inner.mesh.as_ref().unwrap().clone();
//                 }
//             };
//             mesh.apply(&self.transform);
//             let aabb = mesh.aabb();

//             inner.mesh = Some(Arc::new(mesh));
//             inner.dirty_flag = false;
//             inner.aabb = aabb;
//         }
//         inner.mesh.as_ref().unwrap().clone()
//     }

//     fn intersect(&self, ray: Ray) -> IntersectResult {
//         todo!()
//     }

//     fn info(&self) -> GeometryInfo {
//         GeometryInfo { is_static: self.is_static, is_instance: false }
//     }

//     fn transform(&self) -> &Transform {
//         &self.transform
//     }

//     // fn mesh_version(&self) -> u64 {
//     //     let inner = self.inner.lock().unwrap();
//     //     inner.version
//     // }

//     fn aabb(&self) -> Option<BoundBox> {
//         let mut inner = self.inner.lock().unwrap();
//         inner.aabb.clone()
//     }
// }

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
