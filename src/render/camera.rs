use std::cell::RefCell;

use nalgebra::Unit;
use winit::event::{ElementState, MouseButton};

use crate::types::{Mat4x4f, Point3, Quaternion, Rotation3, Vec2f, Vec3f, Vec4f};

use super::{executor::ExecutorInputEvent, transform::TransformBuilder, Transform};

#[derive(Debug)]
struct Inner {
    projection: Mat4x4f,
    view: Mat4x4f,
    from: Vec3f,
    to: Vec3f,
    up: Vec3f,
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
                from: Vec3f::new(1f32, 1f32, 1f32),
                to: Vec3f::new(0f32, 0f32, 0f32),
                up: Vec3f::new(0f32, 1f32, 0f32),
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
    pub fn look_at(&self, from: Vec3f, to: Vec3f, up: Vec3f) {
        let mut inner = self.inner.borrow_mut();
        inner.from = from;
        inner.to = to;
        inner.up = up;
        let from = from.into();
        let to = to.into();
        inner.view = Mat4x4f::look_at_rh(&from, &to, &up);
    }
    pub fn from(&self) -> Vec3f {
        let inner = self.inner.borrow();
        inner.from
    }
    pub fn to(&self) -> Vec3f {
        let inner = self.inner.borrow();
        inner.to
    }
    pub fn up(&self) -> Vec3f {
        let inner = self.inner.borrow();
        inner.up
    }
    pub fn right(&self) -> Vec3f {
        let inner = self.inner.borrow();
        (inner.from - inner.to).cross(&inner.up)
    }
}

pub trait CameraController {
    fn on_input(&mut self, event: ExecutorInputEvent);
}

pub struct TrackballCameraController<'a> {
    camera: &'a Camera,
    down_pos: Option<Vec2f>,
    last_pos: Vec2f,
}

impl<'a> TrackballCameraController<'a> {
    pub fn new(camera: &'a Camera) -> Self {
        Self {
            camera,
            down_pos: None,
            last_pos: Vec2f::default(),
        }
    }
}

impl<'a> CameraController for TrackballCameraController<'a> {
    fn on_input(&mut self, event: ExecutorInputEvent) {
        match event {
            crate::event::InputEvent::KeyboardInput {
                device_id,
                input,
                is_synthetic,
            } => todo!(),
            crate::event::InputEvent::ModifiersChanged(_) => todo!(),
            crate::event::InputEvent::CursorMoved {
                device_id,
                position,
            } => {
                let last_pos = Vec2f::new(position.x as f32, position.y as f32);
                let delta = last_pos - self.last_pos;
                self.last_pos = last_pos;
                if self.down_pos.is_none() {
                    return;
                }

                let from = self.camera.from();
                let to = self.camera.to();
                let up = self.camera.up();
                let right = self.camera.right();
                let unit_up = Unit::new_unchecked(up.normalize());
                let unit_right = Unit::new_unchecked(right.normalize());

                let q =
                    Quaternion::from_axis_angle(&unit_up, -delta.x * 0.001 * std::f32::consts::PI);
                let q2 = Quaternion::from_axis_angle(
                    &unit_right,
                    delta.y * 0.001 * std::f32::consts::PI,
                );
                let q = q * q2;

                let target = q * (from - to) + to;

                self.camera.look_at(target, to, up);
            }
            crate::event::InputEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => {
                let from = self.camera.from();
                let to = self.camera.to();
                let up = self.camera.up();

                let vector = from - to;
                let new_offset = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
                let dist = (new_offset * 0.05f32).max(-0.5f32).min(0.5f32);
                let new_from = from - (vector * dist);

                self.camera.look_at(new_from, to, up);
            }
            crate::event::InputEvent::MouseInput {
                device_id,
                state,
                button,
            } => {
                if MouseButton::Left == button {
                    if state == ElementState::Pressed {
                        self.down_pos = Some(self.last_pos);
                    } else {
                        self.down_pos = None;
                    }
                }
            }
            _ => (),
        }
    }
}
