use std::cell::RefCell;

use crate::types::{Mat4x4f, Point3, Quaternion, Vec3f, Vec4f};

use super::{executor::InputEvent, Transform};

#[derive(Debug)]
struct Inner {
    projection: Mat4x4f,
    view: Mat4x4f,
    orthographic: bool,
}

#[derive(Debug)]
pub struct Camera {
    inner: RefCell<Inner>,
}

impl Camera {
    pub fn new() -> Self {
        Self {
            inner: Inner {
                projection: Mat4x4f::identity(),
                view: Mat4x4f::identity(),
                orthographic: true,
            }
            .into(),
        }
    }
    pub fn mat_vp(&self) -> (Mat4x4f, Mat4x4f) {
        let inner = self.inner.borrow_mut();
        (inner.projection.clone(), inner.view.clone())
    }

    pub fn vp(&self) -> Mat4x4f {
        let inner = self.inner.borrow_mut();
        inner.projection * inner.view
    }

    pub fn make_orthographic(&self, rect: Vec4f, near: f32, far: f32) {
        let mut inner = self.inner.borrow_mut();
        inner.projection =
            Mat4x4f::new_orthographic(rect.x, rect.z, rect.w, rect.y, near, far).into()
    }
    pub fn make_perspective(&self, aspect: f32, fovy: f32, znear: f32, zfar: f32) {
        let mut inner = self.inner.borrow_mut();
        inner.projection = Mat4x4f::new_perspective(aspect, fovy, znear, zfar).into()
    }
    pub fn look_at(&self, from: Point3<f32>, to: Point3<f32>, up: Vec3f) {
        let mut inner = self.inner.borrow_mut();
        inner.view = Mat4x4f::look_at_rh(&from, &to, &up);
    }
}

pub trait CameraController {
    fn on_input(&mut self, event: InputEvent);
}

pub struct EventController<'a> {
    camera: &'a Camera,
}

impl<'a> EventController<'a> {
    pub fn new(camera: &'a Camera) -> Self {
        Self { camera }
    }
}

impl<'a> CameraController for EventController<'a> {
    fn on_input(&mut self, event: InputEvent) {}
}
