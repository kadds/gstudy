use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::event::InputEvent;

use self::{orbit::OrbitControllerFactory, trackball::TrackballControllerFactory};

use super::Camera;

pub trait ControllerFactory: Send + Sync {
    fn create(&self, camera: Arc<Camera>) -> Box<RefCell<dyn CameraController>>;
    fn name(&self) -> String;
}

pub trait CameraController {
    fn on_input(&mut self, event: &InputEvent);
}

pub mod orbit;
pub mod trackball;

pub struct CameraControllerFactory {
    factory: HashMap<String, Box<dyn ControllerFactory>>,
}

impl CameraControllerFactory {
    pub fn new() -> Self {
        let mut s = Self {
            factory: HashMap::new(),
        };
        s.add_inner(Box::new(OrbitControllerFactory));
        s.add_inner(Box::new(TrackballControllerFactory));

        s
    }

    fn add_inner(&mut self, factory: Box<dyn ControllerFactory>) {
        let name = factory.name();
        self.add(name, factory);
    }

    pub fn add<S: Into<String>>(&mut self, name: S, factory: Box<dyn ControllerFactory>) {
        self.factory.insert(name.into(), factory);
    }

    pub fn create(
        &self,
        name: &str,
        camera: Arc<Camera>,
    ) -> Option<Box<RefCell<dyn CameraController>>> {
        self.factory.get(name).map(|v| v.create(camera))
    }

    pub fn list(&self) -> Vec<String> {
        self.factory.keys().cloned().collect()
    }
}

#[derive(Debug)]
pub struct ControllerDriver {
    mouse_enabled: bool,
    keyboard_enabled: bool,
    capture_mouse: bool,
}
impl Default for ControllerDriver {
    fn default() -> Self {
        Self {
            mouse_enabled: true,
            keyboard_enabled: true,
            capture_mouse: false,
        }
    }
}

impl ControllerDriver {
    pub fn on_input(&mut self, event: &InputEvent) -> Option<()> {
        match event {
            crate::event::InputEvent::CursorMoved {
                logical: _,
                physical: _,
            } => {
                if !self.capture_mouse {
                    if !self.mouse_enabled {
                        return None;
                    }
                }
            }
            crate::event::InputEvent::KeyboardInput(i) => {
                if i.state.is_pressed() {
                    if !self.keyboard_enabled {
                        return None;
                    }
                }
            }
            crate::event::InputEvent::MouseWheel { delta: _ } => {
                if !self.mouse_enabled {
                    return None;
                }
            }
            crate::event::InputEvent::MouseInput { state, button: _ } => {
                if state.is_pressed() {
                    if !self.mouse_enabled {
                        return None;
                    }
                    self.capture_mouse = true;
                } else {
                    self.capture_mouse = false;
                }
            }
            crate::event::InputEvent::CaptureMouseInputIn => {
                self.mouse_enabled = false;
            }
            crate::event::InputEvent::CaptureMouseInputOut => {
                self.mouse_enabled = true;
            }
            crate::event::InputEvent::CaptureKeyboardInputIn => {
                self.keyboard_enabled = false;
            }
            crate::event::InputEvent::CaptureKeyboardInputOut => {
                self.keyboard_enabled = true;
            }
            _ => (),
        }

        Some(())
    }
}
