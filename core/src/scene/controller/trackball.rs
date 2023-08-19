use std::{cell::RefCell, sync::Arc};

use nalgebra::Unit;

use crate::{
    event::InputEvent,
    scene::Camera,
    types::{Quaternion, Vec2f},
};

use super::{CameraController, ControllerFactory};

pub struct TrackballControllerFactory;

impl ControllerFactory for TrackballControllerFactory {
    fn create(&self, camera: Arc<Camera>) -> Box<std::cell::RefCell<dyn CameraController>> {
        Box::new(RefCell::new(TrackballCameraController::new(camera)))
    }
    fn name(&self) -> String {
        "Trackball".into()
    }
}

#[derive(Debug, Eq, PartialEq)]
enum State {
    None,
    Orbiting,
    Zooming,
    Panning,
}

pub struct TrackballCameraController {
    camera: Arc<Camera>,
    down_pos: Option<Vec2f>,
    state: State,
    last_pos: Vec2f,
}

impl TrackballCameraController {
    pub fn new(camera: Arc<Camera>) -> Self {
        Self {
            camera,
            down_pos: None,
            state: State::None,
            last_pos: Vec2f::default(),
        }
    }

    fn orbit(&mut self, offset: &Vec2f) {
        let dt = 0.01f32;

        let from = self.camera.from();
        let to = self.camera.to();
        let up = self.camera.up();
        let right = self.camera.right();
        let unit_up = Unit::new_unchecked(up.normalize());
        let unit_right = Unit::new_unchecked(right.normalize());
        let dist = nalgebra::distance(&from.into(), &to.into());

        let delta_theta_y = offset.y * 0.1 * dt * std::f32::consts::PI;

        let theta = f32::asin((from.y - to.y) / dist);
        if theta >= (std::f32::consts::FRAC_PI_2 - delta_theta_y) && offset.y > 0f32 {
            return;
        }
        if theta <= (-std::f32::consts::FRAC_PI_2 - delta_theta_y) && offset.y < 0f32 {
            return;
        }

        let q = Quaternion::from_axis_angle(&unit_up, -offset.x * 0.1 * dt * std::f32::consts::PI);
        let q2 = Quaternion::from_axis_angle(&unit_right, delta_theta_y);
        let q = q * q2;

        let target = q * (from - to) + to;

        self.camera.look_at(target, to, up);
    }

    fn zoom(&mut self, offset: &Vec2f) {
        let from = self.camera.from();
        let to = self.camera.to();
        let up = self.camera.up();

        let vector = from - to;
        let new_offset = offset.y;
        let dist = (new_offset * 0.01f32).max(-0.2f32).min(0.2f32);
        let new_from = from - (vector * dist);

        self.camera.look_at(new_from, to, up);
    }

    fn pan(&mut self, offset: &Vec2f) {
        let from = self.camera.from();
        let to = self.camera.to();
        let up = self.camera.up();
        let right = self.camera.right().normalize();

        let dist = nalgebra::distance(&from.into(), &to.into());

        let o = right * (offset.x * 0.001f32 * dist);
        let p = up * (offset.y * 0.001f32 * dist);

        let from = from + o + p;
        let to = to + o + p;

        self.camera.look_at(from, to, up);
    }
}

impl CameraController for TrackballCameraController {
    fn on_input(&mut self, event: &InputEvent) {
        match event {
            crate::event::InputEvent::CursorMoved {
                logical: _,
                physical,
            } => {
                if State::None == self.state {
                    self.last_pos = *physical;
                    return;
                }
                let offset = *physical - self.last_pos;
                self.last_pos = *physical;
                match self.state {
                    State::Orbiting => {
                        self.orbit(&offset);
                    }
                    State::Zooming => {
                        self.zoom(&offset);
                    }
                    State::Panning => {
                        self.pan(&offset);
                    }
                    _ => {}
                }
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
                        self.state = State::Orbiting;
                        self.down_pos = Some(self.last_pos);
                    } else {
                        self.state = State::None;
                        self.down_pos = None;
                    }
                } else if button.is_right() {
                    if state.is_pressed() {
                        self.state = State::Panning;
                        self.down_pos = Some(self.last_pos);
                    } else {
                        self.state = State::None;
                        self.down_pos = None;
                    }
                } else if button.is_middle() {
                    if state.is_pressed() {
                        self.state = State::Zooming;
                        self.down_pos = Some(self.last_pos);
                    } else {
                        self.state = State::None;
                        self.down_pos = None;
                    }
                }
            }
            _ => (),
        }
    }
}
