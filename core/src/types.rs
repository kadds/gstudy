use std::ops::Add;

use nalgebra::{SMatrix, SimdPartialOrd, Vector2, Vector3, Vector4};
use ordered_float::OrderedFloat;

use crate::{
    debug::DebugMeshGenerator,
    mesh::builder::{MeshBuilder, MeshPropertiesBuilder, MeshPropertyType},
};

pub type Mat3x3f = SMatrix<f32, 3, 3>;
pub type Mat4x4f = SMatrix<f32, 4, 4>;

pub type Vec2<T> = Vector2<T>;
pub type Vec3<T> = Vector3<T>;
pub type Vec4<T> = Vector4<T>;
pub type Vec2f = Vec2<f32>;
pub type Vec3f = Vec3<f32>;
pub type Vec4f = Vec4<f32>;

pub type Vec3u = Vec3<u32>;

pub type Point2<T> = nalgebra::Point2<T>;
pub type Point3<T> = nalgebra::Point3<T>;
pub type Point4<T> = nalgebra::Point4<T>;
pub type Rectu = Point4<u32>;
pub type Size = Point2<u32>;
pub type Sizef = Point2<f32>;
pub type Color = Vec4f;
pub type Quaternion = nalgebra::UnitQuaternion<f32>;
pub type Rotation3 = nalgebra::geometry::Rotation3<f32>;
pub type Translation3 = nalgebra::geometry::Translation3<f32>;
pub type Transform3 = nalgebra::geometry::Transform3<f32>;

#[inline]
fn to_round_u8(res: &Vec4f, idx: usize) -> u8 {
    unsafe { res.vget_unchecked(idx).round() as u8 }
}

#[inline]
pub fn to_rgba_u8(vec: &Vec4f) -> [u8; 4] {
    let res = vec.scale(255f32);
    [
        to_round_u8(&res, 0),
        to_round_u8(&res, 1),
        to_round_u8(&res, 2),
        to_round_u8(&res, 3),
    ]
}

pub trait Bound {
    fn in_frustum(&self, frustum: &Frustum) -> bool;
}

#[derive(Debug, Default)]
pub enum Boundary {
    #[default]
    None,
    AABB(BoundBox),
    OBB,
}

impl Boundary {
    pub fn distance(&self, pos: &Point3<f32>) -> OrderedFloat<f32> {
        match self {
            Boundary::None => OrderedFloat::<f32>(0f32),
            Boundary::AABB(aabb) => {
                let a = aabb.center().into();
                OrderedFloat::<f32>(nalgebra::distance_squared(&a, pos))
            }
            Boundary::OBB => todo!(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct BoundBox {
    val: Option<(Vec3f, Vec3f)>,
}

impl BoundBox {
    pub fn new(min: Vec3f, max: Vec3f) -> Self {
        let minx = min.x.min(max.x);
        let maxx = min.x.max(max.x);
        let miny = min.y.min(max.y);
        let maxy = max.y.max(max.y);
        let minz = min.z.min(max.z);
        let maxz = min.z.max(max.z);

        Self {
            val: Some((Vec3f::new(minx, miny, minz), Vec3f::new(maxx, maxy, maxz))),
        }
    }
    pub fn min(&self) -> &Vec3f {
        &self.val.as_ref().unwrap().0
    }
    pub fn max(&self) -> &Vec3f {
        &self.val.as_ref().unwrap().1
    }
    pub fn center(&self) -> Vec3f {
        let v = self.val.as_ref().unwrap();
        let p3 = nalgebra::center(&v.0.into(), &v.1.into());
        Vec3f::new(p3.x, p3.y, p3.z)
    }

    pub fn size(&self) -> Vec3f {
        let v = self.val.as_ref().unwrap();
        (v.1 - v.0).abs()
    }
    pub fn mul_mut(&mut self, t: &Mat4x4f) {
        if let Some(v) = &mut self.val {
            v.0 = (t * Vec4f::new(v.0.x, v.0.y, v.0.z, 1.0f32)).xyz();
            v.1 = (t * Vec4f::new(v.1.x, v.1.y, v.1.z, 1.0f32)).xyz();
        }
    }
}

impl Bound for BoundBox {
    fn in_frustum(&self, _frustum: &Frustum) -> bool {
        true
    }
}

impl Add<&BoundBox> for &BoundBox {
    type Output = BoundBox;

    fn add(self, rhs: &BoundBox) -> Self::Output {
        if let Some(lhs) = self.val {
            if let Some(rhs) = rhs.val {
                let min = lhs.0.simd_min(rhs.0);
                let max = lhs.1.simd_max(rhs.1);

                BoundBox::new(min, max)
            } else {
                BoundBox::new(lhs.0, lhs.1)
            }
        } else {
            if let Some(rhs) = rhs.val {
                BoundBox::new(rhs.0, rhs.1)
            } else {
                BoundBox { val: None }
            }
        }
    }
}

impl Add<&Vec3f> for &BoundBox {
    type Output = BoundBox;

    fn add(self, rhs: &Vec3f) -> Self::Output {
        if let Some(val) = self.val {
            let min = val.0.simd_min(*rhs);
            let max = val.1.simd_max(*rhs);

            BoundBox::new(min, max)
        } else {
            BoundBox::new(*rhs, *rhs)
        }
    }
}

pub struct BoundSphere {
    radius: f32,
    center: Vec3f,
}

impl BoundSphere {
    pub fn new(center: Vec3f, radius: f32) -> Self {
        Self { radius, center }
    }
}

impl Bound for BoundSphere {
    fn in_frustum(&self, _frustum: &Frustum) -> bool {
        true
    }
}

pub struct Plane {
    pos: Vec4f,
}

impl Plane {
    pub fn new(normal: Vec3f, distance: f32) -> Self {
        Self {
            pos: Vec4f::new(normal.x, normal.y, normal.z, distance),
        }
    }

    pub fn normal(&self) -> Vec3f {
        self.pos.xyz()
    }
}

pub struct Frustum {
    // pub near_lt: Vec3f,
    // pub near_rt: Vec3f,
    // pub near_lb: Vec3f,
    // pub near_rb: Vec3f,

    // pub far_lt: Vec3f,
    // pub far_rt: Vec3f,
    // pub far_lb: Vec3f,
    // pub far_rb: Vec3f,

    // pub position: Vec3f,
    pub pos: [Vec3f; 12],
}

impl Frustum {
    pub fn new(frustum: &[Vec3f; 8], position: Vec3f, to: Vec3f, up: Vec3f) -> Self {
        // let near =
        let mut pos: [_; 12] = [Vec3f::default(); 12];
        (&mut pos[..8]).copy_from_slice(frustum);
        pos[8] = position;
        pos[9] = to;
        pos[10] = position + up;
        let right = (position - to).normalize().cross(&up);
        pos[11] = position + right;

        Self { pos: pos }
    }
}

impl DebugMeshGenerator for Frustum {
    fn generate(&self, color: Color) -> crate::mesh::Mesh {
        let mut mesh_builder = MeshBuilder::default();
        let mut properties_builder = MeshPropertiesBuilder::default();
        let property = MeshPropertyType::new::<Color>("color");
        properties_builder.add_property(property);

        mesh_builder.add_position_vertices3(&self.pos[..]);
        mesh_builder.add_indices32(&[0, 1, 1, 3, 2, 3, 0, 2]);
        mesh_builder.add_indices32(&[4, 5, 5, 7, 6, 7, 4, 6]);
        mesh_builder.add_indices32(&[0, 4, 1, 5, 3, 7, 2, 6]);
        mesh_builder.add_indices32(&[8, 0, 8, 1, 8, 2, 8, 3]);
        mesh_builder.add_indices32(&[8, 9, 8, 10, 8, 11]);

        let pos_c = Color::new(1.0f32, 0.4f32, 0.5f32, 1.0f32);
        let pos_to = Color::new(1.0f32, 1f32, 1.0f32, 1.0f32);
        let pos_up = Color::new(0.3f32, 1f32, 0.3f32, 1.0f32);
        let pos_right = Color::new(0.5f32, 0.4f32, 1f32, 1.0f32);

        properties_builder.add_property_data(
            property,
            &[
                color, color, color, color, color, color, color, color, pos_c, pos_to, pos_up,
                pos_right,
            ],
        );

        mesh_builder.set_properties(properties_builder.build());

        mesh_builder.build().unwrap()
    }
}
