use nalgebra::{SMatrix, Vector2, Vector3, Vector4};

pub type Mat3x3f = SMatrix<f32, 3, 3>;
pub type Mat4x4f = SMatrix<f32, 4, 4>;

pub type Vec2<T> = Vector2<T>;
pub type Vec3<T> = Vector3<T>;
pub type Vec4<T> = Vector4<T>;
pub type Vec2f = Vec2<f32>;
pub type Vec3f = Vec3<f32>;
pub type Vec4f = Vec4<f32>;
pub type Point2<T> = nalgebra::Point2<T>;
pub type Point3<T> = nalgebra::Point3<T>;
pub type Point4<T> = nalgebra::Point4<T>;
pub type Size = Point2<u32>;
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
