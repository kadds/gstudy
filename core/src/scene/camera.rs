use std::{
    cell::RefCell,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use nalgebra::Unit;

use crate::{
    context::RContext,
    event::{self, InputEvent},
    types::{Color, Mat4x4f, Point3, Quaternion, Rotation3, Vec2f, Vec3f, Vec4f},
};

// pub type OptionalTexture = Option<Texture>;

#[derive(Debug)]
struct Inner {
    projection: Mat4x4f,
    view: Mat4x4f,
    from: Vec3f,
    to: Vec3f,
    up: Vec3f,
    orthographic: bool,
    ortho_size: Vec2f,
    fovy: f32,
    near: f32,
    far: f32,
    change_id: u64,
    // attachment: Option<RenderAttachment>,
}

// #[derive(Debug, Clone)]
// pub struct RenderAttachment {
//     texture: Option<(OptionalTexture, OptionalTexture)>,
//     clear_color: Option<Color>,
//     clear_depth: Option<f32>,
//     format: wgpu::TextureFormat,
//     id: u64,
// }

// impl RenderAttachment {
//     pub fn new_with_color_depth(
//         id: u64,
//         color_attachment: Texture,
//         depth_attachment: Texture,
//         clear_color: Option<Color>,
//         clear_depth: Option<f32>,
//         format: wgpu::TextureFormat,
//     ) -> Self {
//         Self {
//             texture: Some((Some(color_attachment), Some(depth_attachment))),
//             clear_color,
//             clear_depth,
//             format,
//             id,
//         }
//     }
//     pub fn set_clear_color(&mut self, color: Option<Color>) {
//         self.clear_color = color;
//     }
//     pub fn set_depth(&mut self, depth: Option<f32>) {
//         self.clear_depth = depth;
//     }

//     pub fn color_attachment(&self) -> Option<&Texture> {
//         self.texture.as_ref()?.0.as_ref().map(|v| v)
//     }
//     pub fn depth_attachment(&self) -> Option<&Texture> {
//         self.texture.as_ref()?.1.as_ref().map(|v| v)
//     }
//     pub fn clear_color(&self) -> Option<Color> {
//         self.clear_color
//     }
//     pub fn clear_depth(&self) -> Option<f32> {
//         self.clear_depth
//     }
//     pub fn format(&self) -> wgpu::TextureFormat {
//         self.format
//     }
//     pub fn id(&self) -> u64 {
//         self.id
//     }
// }

pub struct Camera {
    inner: Mutex<Inner>,
    id: u64,
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
        .field("id", &self.id)
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
                near: 0.01f32,
                far: f32::MAX,
                fovy: 0f32,
                change_id: 0,
                // attachment: None,
            }
            .into(),
            id: context.alloc_camera_id(),
        }
    }
    pub fn id(&self) -> u64 {
        self.id
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
    fn on_input(&mut self, delta: f32, event: InputEvent);
}

pub struct TrackballCameraController {
    camera: Arc<Camera>,
    down_pos: Option<Vec2f>,
    last_pos: Vec2f,
}

impl TrackballCameraController {
    pub fn new(camera: Arc<Camera>) -> Self {
        Self {
            camera,
            down_pos: None,
            last_pos: Vec2f::default(),
        }
    }
}

impl CameraController for TrackballCameraController {
    fn on_input(&mut self, dt: f32, event: InputEvent) {
        match event {
            crate::event::InputEvent::KeyboardInput(input) => match input.vk {
                event::VirtualKeyCode::W => {}
                event::VirtualKeyCode::A => {}
                event::VirtualKeyCode::S => {}
                event::VirtualKeyCode::D => {}
                _ => (),
            },
            crate::event::InputEvent::CursorMoved { logical, physical } => {
                let last_pos = Vec2f::new(physical.x as f32, physical.y as f32);
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
