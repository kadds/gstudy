use std::{cell::RefCell, ops::Mul};

use crate::types::*;

#[derive(Debug, Clone)]
pub struct Transform {
    mat: Mat4x4f,
    translate: Vec3f,
    scale: Vec3f,
    rotate: Quaternion,
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

        t.mat = t.rotate.to_homogeneous();
        t.mat.append_nonuniform_scaling_mut(&t.scale);
        t.mat.append_translation_mut(&t.translate);

        t
    }

    pub fn translate(mut self, offset: Vec3f) -> Self {
        self.inner.translate += offset;
        self
    }
    pub fn scale(mut self, scale: Vec3f) -> Self {
        self.inner.scale += scale;
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
        let mat = self.mat.clone();
        vertices.map(move |vertex| {
            let v = Vec4f::new(vertex.x, vertex.y, vertex.z, 0f32);
            let v4 = mat * v;
            Vec3f::new(v4.x, v4.y, v4.z)
        })
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            mat: Mat4x4f::identity(),
            translate: Vec3f::default(),
            scale: Vec3f::identity(),
            rotate: Quaternion::identity(),
        }
    }
}
