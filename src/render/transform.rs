use std::{cell::RefCell, ops::Mul};

use crate::types::*;

#[derive(Debug, Clone)]
pub struct Transform {
    mat: Mat4x4f,
    translate: Vec3f,
    scale: Vec3f,
    rotate: Quaternion,
    flag: bool,
}

impl Transform {
    pub fn mat(&mut self) -> &Mat4x4f {
        if self.flag {
            self.mat = self.rotate.clone().to_homogeneous();
            self.mat.append_nonuniform_scaling_mut(&self.scale);
            self.mat.append_translation_mut(&self.translate);
            self.flag = false;
        }
        &self.mat
    }
    pub fn set_translate(&mut self, translate: Vec3f) {
        self.translate = translate;
        self.flag = true;
    }

    pub fn add_translate(&mut self, offset: &Vec3f) {
        self.translate += offset;
        self.flag = true;
    }

    pub fn set_scale(&mut self, scale: Vec3f) {
        self.scale = scale;
        self.flag = true;
    }

    pub fn add_scale(&mut self, scale: &Vec3f) {
        self.scale += scale;
        self.flag = true;
    }

    pub fn set_rotate(&mut self, rotate: Quaternion) {
        self.rotate = rotate;
        self.flag = true;
    }

    pub fn add_rotate(&mut self, offset: &Quaternion) {
        self.rotate *= offset;
        self.flag = true;
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            mat: Mat4x4f::identity(),
            translate: Vec3f::default(),
            scale: Vec3f::identity(),
            rotate: Quaternion::identity(),
            flag: false,
        }
    }
}
