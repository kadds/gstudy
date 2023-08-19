use std::{cell::RefCell, collections::HashMap, sync::Arc};

use crate::event::InputEvent;

use self::{
    orbit::{OrbitCameraController, OrbitControllerFactory},
    trackball::TrackballControllerFactory,
};

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
