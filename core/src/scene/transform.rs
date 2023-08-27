use std::{fmt::Debug, ops::Mul};

use crate::types::*;

#[derive(Clone)]
pub struct Transform {
    mat: Mat4x4f,
    translate: Vec3f,
    scale: Vec3f,
    rotate: Quaternion,
}

impl Debug for Transform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transform")
            .field("translate", &self.translate)
            .field("scale", &self.scale)
            .field("rotate", &self.rotate.to_rotation_matrix())
            .finish()
    }
}

pub struct TransformBuilder {
    inner: Transform,
}

impl TransformBuilder {
    pub fn new() -> Self {
        Self {
            inner: Transform::default(),
        }
    }

    pub fn build(self) -> Transform {
        let mut t = self.inner;

        t.mat.append_nonuniform_scaling_mut(&t.scale);
        t.mat *= t.rotate.to_homogeneous();
        t.mat.append_translation_mut(&t.translate);

        t
    }

    pub fn translate(mut self, offset: Vec3f) -> Self {
        self.inner.translate += offset;
        self
    }
    pub fn scale(mut self, scale: Vec3f) -> Self {
        self.inner.scale = scale.component_mul(&self.inner.scale);
        self
    }
    pub fn rotate(mut self, rotate: Quaternion) -> Self {
        self.inner.rotate *= rotate;
        self
    }
}

impl Transform {
    pub fn builder(self) -> TransformBuilder {
        TransformBuilder { inner: self }
    }

    pub fn apply(&self, vertex: Vec3f) -> Vec3f {
        let v = Vec4f::new(vertex.x, vertex.y, vertex.z, 0f32);
        let v4 = self.mat * v;
        Vec3f::new(v4.x, v4.y, v4.z)
    }
    pub fn apply_batch(
        &self,
        vertices: impl Iterator<Item = Vec3f>,
    ) -> impl Iterator<Item = Vec3f> {
        let mat = self.mat;
        vertices.map(move |vertex| {
            let v = Vec4f::new(vertex.x, vertex.y, vertex.z, 0f32);
            let v4 = mat * v;
            Vec3f::new(v4.x, v4.y, v4.z)
        })
    }
    pub fn mat(&self) -> &Mat4x4f {
        &self.mat
    }
    pub fn mul_mut(&mut self, t: &Transform) {
        self.mat = t.mat * self.mat;
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            mat: Mat4x4f::identity(),
            translate: Vec3f::default(),
            scale: Vec3f::new(1f32, 1f32, 1f32),
            rotate: Quaternion::identity(),
        }
    }
}

impl Mul<&Transform> for &Transform {
    type Output = Transform;

    fn mul(self, rhs: &Transform) -> Self::Output {
        let t = self.translate + rhs.translate;
        let r = self.rotate * rhs.rotate;
        let s = self.scale.component_mul(&rhs.scale);
        TransformBuilder::new()
            .translate(t)
            .rotate(r)
            .scale(s)
            .build()
    }
}
