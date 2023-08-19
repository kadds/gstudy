use std::{
    fmt::Debug,
    ops::Mul,
    sync::{Arc, Mutex},
    time::Instant,
};

use nalgebra::Unit;

use crate::{
    context::RContext,
    event::{self, InputEvent},
    types::{Frustum, Mat4x4f, Quaternion, Vec2f, Vec3f, Vec4f},
};

// pub type OptionalTexture = Option<Texture>;

#[derive(Debug, Clone)]
struct Inner {
    projection: Mat4x4f,
    view: Mat4x4f,
    from: Vec3f,
    to: Vec3f,
    up: Vec3f,
    aspect: f32,
    orthographic: bool,
    ortho_size: Vec2f,
    fovy: f32,
    near: f32,
    far: f32,
    change_id: u64,
}

pub struct Camera {
    inner: Mutex<Inner>,
}

impl Clone for Camera {
    fn clone(&self) -> Self {
        Self {
            inner: Mutex::new(self.inner.lock().unwrap().clone()),
        }
    }
}

impl Debug for Camera {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let d = self.inner.lock().unwrap();
        let mut sd = f.debug_struct("Camera");
        if d.orthographic {
            sd.field("type", &"o")
                .field("width", &d.ortho_size.x)
                .field("height", &d.ortho_size.y)
        } else {
            sd.field("type", &"p")
        }
        .field("from", &d.from)
        .field("to", &d.to)
        .field("up", &d.up)
        .field("fovy", &(d.fovy / std::f32::consts::PI * 180f32))
        .field("near", &d.near)
        .field("far", &d.far)
        .finish()
    }
}

impl Camera {
    pub fn new(context: &RContext) -> Self {
        Self {
            inner: Inner {
                projection: Mat4x4f::identity(),
                view: Mat4x4f::identity(),
                orthographic: true,
                from: Vec3f::new(1f32, 1f32, 1f32),
                to: Vec3f::new(0f32, 0f32, 0f32),
                up: Vec3f::new(0f32, 1f32, 0f32),
                ortho_size: Vec2f::zeros(),
                aspect: 0f32,
                near: 0.01f32,
                far: f32::MAX,
                fovy: 0f32,
                change_id: 0,
            }
            .into(),
        }
    }

    pub fn frustum_worldspace(&self) -> Frustum {
        let inner = self.inner.lock().unwrap();
        let near = inner.near;
        let far = inner.far;
        let fov = inner.fovy;
        let asp = inner.aspect;
        let deg_y = fov.tan();
        let deg_x = deg_y * asp;
        let world = inner.view;

        let f0 = world * Vec4f::new(-deg_x, -deg_y, 1f32, 1f32);
        let f1 = world * Vec4f::new(-deg_x, deg_y, 1f32, 1f32);
        let f2 = world * Vec4f::new(deg_x, -deg_y, 1f32, 1f32);
        let f3 = world * Vec4f::new(deg_x, deg_y, 1f32, 1f32);

        let pos = inner.from;

        Frustum::new([
            pos + (near * f1).xyz(),
            pos + (near * f3).xyz(),
            pos + (near * f0).xyz(),
            pos + (near * f2).xyz(),
            pos + (far * f1).xyz(),
            pos + (far * f3).xyz(),
            pos + (far * f0).xyz(),
            pos + (far * f2).xyz(),
        ])
    }

    pub fn change_id(&self) -> u64 {
        let inner = self.inner.lock().unwrap();
        inner.change_id
    }

    pub fn vp(&self) -> Mat4x4f {
        let inner = self.inner.lock().unwrap();
        inner.projection * inner.view
    }

    pub fn make_orthographic(&self, rect: Vec4f, near: f32, far: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.projection =
            Mat4x4f::new_orthographic(rect.x, rect.z, rect.w, rect.y, near, far).into();
        inner.orthographic = true;
        inner.ortho_size = Vec2f::new(rect.z - rect.x, rect.w - rect.y);
        inner.change_id += 1;
    }
    pub fn make_perspective(&self, aspect: f32, fovy: f32, znear: f32, zfar: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.projection = Mat4x4f::new_perspective(aspect, fovy, znear, zfar).into();
        inner.fovy = fovy;
        inner.near = znear;
        inner.far = zfar;
        inner.aspect = aspect;
        inner.orthographic = false;
        inner.ortho_size = Vec2f::zeros();
        inner.change_id += 1;
    }
    pub fn remake_perspective(&self, aspect: f32) {
        let mut inner = self.inner.lock().unwrap();
        inner.projection =
            Mat4x4f::new_perspective(aspect, inner.fovy, inner.near, inner.far).into();
        inner.orthographic = false;
        inner.ortho_size = Vec2f::zeros();
        inner.aspect = aspect;
        inner.change_id += 1;
    }

    pub fn is_perspective(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        !inner.orthographic
    }

    pub fn look_at(&self, from: Vec3f, to: Vec3f, up: Vec3f) {
        let mut inner = self.inner.lock().unwrap();
        inner.from = from;
        inner.to = to;
        inner.up = up;
        let from = from.into();
        let to = to.into();
        inner.view = Mat4x4f::look_at_rh(&from, &to, &up);
        inner.change_id += 1;
    }
    pub fn from(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        inner.from
    }
    pub fn to(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        inner.to
    }
    pub fn up(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        inner.up
    }
    pub fn right(&self) -> Vec3f {
        let inner = self.inner.lock().unwrap();
        (inner.from - inner.to).cross(&inner.up)
    }

    pub fn width_height(&self) -> Vec2f {
        let inner = self.inner.lock().unwrap();
        inner.ortho_size
    }

    //     pub fn bind_render_attachment(&self, attachment: RenderAttachment) {
    //         let mut inner = self.inner.lock().unwrap();
    //         inner.attachment = Some(attachment);
    //     }

    //     pub fn take_render_attachment(&self) -> Option<RenderAttachment> {
    //         let mut inner = self.inner.lock().unwrap();
    //         inner.attachment.take()
    //     }

    //     pub fn render_attachment_format(&self) -> wgpu::TextureFormat {
    //         let inner = self.inner.lock().unwrap();
    //         inner.attachment.as_ref().unwrap().format()
    //     }
    // }
}

pub trait CameraController {
    fn on_input(&mut self, event: &InputEvent);
}

pub struct TrackballCameraController {
    camera: Arc<Camera>,
    down_pos: Option<Vec2f>,
    last_pos: Vec2f,
    last_move: Instant,
}

impl TrackballCameraController {
    pub fn new(camera: Arc<Camera>) -> Self {
        Self {
            camera,
            down_pos: None,
            last_pos: Vec2f::default(),
            last_move: Instant::now(),
        }
    }
}

impl CameraController for TrackballCameraController {
    fn on_input(&mut self, event: &InputEvent) {
        match event {
            crate::event::InputEvent::KeyboardInput(input) => match input.vk {
                event::VirtualKeyCode::W => {}
                event::VirtualKeyCode::A => {}
                event::VirtualKeyCode::S => {}
                event::VirtualKeyCode::D => {}
                _ => (),
            },
            crate::event::InputEvent::CursorMoved {
                logical: _,
                physical,
            } => {
                // let last_time = self.last_move;
                // self.last_move = Instant::now();
                let last_pos = Vec2f::new(physical.x, physical.y);
                let delta = last_pos - self.last_pos;
                self.last_pos = last_pos;
                if self.down_pos.is_none() {
                    return;
                }
                // let dt = (self.last_move - last_time).as_secs_f32();
                let dt = 0.01f32;

                let from = self.camera.from();
                let to = self.camera.to();
                let up = self.camera.up();
                let right = self.camera.right();
                let unit_up = Unit::new_unchecked(up.normalize());
                let unit_right = Unit::new_unchecked(right.normalize());
                let dist = nalgebra::distance(&from.into(), &to.into());

                let delta_theta_y = delta.y * 0.1 * dt * std::f32::consts::PI;

                let theta = f32::asin((from.y - to.y) / dist);
                if theta >= (std::f32::consts::FRAC_PI_2 - delta_theta_y) && delta.y > 0f32 {
                    return;
                }
                if theta <= (-std::f32::consts::FRAC_PI_2 - delta_theta_y) && delta.y < 0f32 {
                    return;
                }

                let q = Quaternion::from_axis_angle(
                    &unit_up,
                    -delta.x * 0.1 * dt * std::f32::consts::PI,
                );
                let q2 = Quaternion::from_axis_angle(&unit_right, delta_theta_y);
                let q = q * q2;

                let target = q * (from - to) + to;

                self.camera.look_at(target, to, up);
            }
            crate::event::InputEvent::MouseWheel { delta } => {
                let from = self.camera.from();
                let to = self.camera.to();
                let up = self.camera.up();

                let vector = from - to;
                let new_offset = delta.y;
                let dist = (new_offset * 0.05f32).max(-0.5f32).min(0.5f32);
                let new_from = from - (vector * dist);

                self.camera.look_at(new_from, to, up);
            }
            crate::event::InputEvent::MouseInput { state, button } => {
                if button.is_left() {
                    if state.is_pressed() {
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
