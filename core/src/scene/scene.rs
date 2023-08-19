use bevy_ecs::prelude::*;
use dashmap::DashMap;

use crate::{
    context::{RContextRef, TagId},
    geometry::Geometry,
    material::Material,
    types::{Size, Vec3f, Vec4f},
};
use std::{
    collections::{BTreeMap, HashSet},
    fmt::Debug,
    sync::{Arc, Mutex},
};

use super::{
    sort::{DistanceSorter, MaterialSorter, Sorter, UISceneSorter},
    Camera,
};

pub const LAYER_NORMAL: u64 = 4_000;
pub const LAYER_BACKGROUND: u64 = 10_000;
pub const LAYER_TRANSPARENT: u64 = 20_000;
pub const LAYER_ALPHA_TEST: u64 = 30_000;
pub const LAYER_UI: u64 = 100_000;
pub const UNKNOWN_OBJECT: u64 = 0;

#[derive(Debug)]
pub struct ObjectWrapper {
    pub layer: u64,
    pub object: RenderObject,
}

impl ObjectWrapper {
    pub fn new(layer: u64, object: RenderObject) -> Self {
        Self { layer, object }
    }
    pub fn o(&self) -> &RenderObject {
        &self.object
    }
}

pub type SceneStorage = Arc<DashMap<u64, ObjectWrapper>>;

#[derive(Debug, Default)]
struct SceneCamera {
    cameras: Vec<Arc<Camera>>,
    ui_camera: Option<Arc<Camera>>,
}

pub struct Scene {
    context: RContextRef,

    storage: SceneStorage,

    // reader layer -> objects
    queue: Mutex<BTreeMap<u64, Arc<Mutex<dyn Sorter>>>>,

    cameras: Mutex<SceneCamera>,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("context", &self.context)
            .field("objects", &self.storage)
            .field("cameras", &self.cameras)
            .finish()
    }
}

impl Scene {
    pub fn new(context: RContextRef) -> Self {
        let mut s = Self {
            context,
            storage: SceneStorage::new(DashMap::new()),

            queue: Mutex::new(BTreeMap::new()),

            cameras: Mutex::new(SceneCamera::default()),
        };
        s.add_default_ui_camera();
        s
    }

    fn add_default_ui_camera(&mut self) {
        let ui_camera = Arc::new(Camera::new());
        ui_camera.make_orthographic(Vec4f::new(0f32, 0f32, 1f32, 1f32), 0.1f32, 10f32);
        ui_camera.look_at(
            Vec3f::new(0f32, 0f32, 1f32),
            Vec3f::zeros(),
            Vec3f::new(0f32, 1f32, 0f32),
        );
        self.cameras.lock().unwrap().ui_camera = Some(ui_camera);
    }

    pub fn context(&self) -> RContextRef {
        self.context.clone()
    }

    pub fn set_main_camera(&self, camera: Arc<Camera>) {
        let mut c = self.cameras.lock().unwrap();

        if c.cameras.is_empty() {
            c.cameras.push(camera.clone());
        } else {
            c.cameras[0] = camera.clone();
        }

        let q = self.queue.lock().unwrap();
        for sorter in q.values() {
            sorter.lock().unwrap().set_camera(camera.clone());
        }
    }

    pub fn main_camera_ref(&self) -> Option<Arc<Camera>> {
        let c = self.cameras.lock().unwrap();
        c.cameras.get(0).cloned()
    }

    pub fn set_ui_camera(&mut self, camera: Arc<Camera>) {
        let mut c = self.cameras.lock().unwrap();
        c.ui_camera = Some(camera);
    }

    pub fn ui_camera_ref(&self) -> Arc<Camera> {
        let c = self.cameras.lock().unwrap();
        c.ui_camera.clone().unwrap()
    }

    pub fn object_size(&self) -> usize {
        self.storage.len()
    }

    pub fn add(&self, object: RenderObject) -> u64 {
        if object.is_alpha_test() {
            self.add_with(object, LAYER_ALPHA_TEST)
        } else if object.is_blend() {
            self.add_with(object, LAYER_TRANSPARENT)
        } else {
            self.add_with(object, LAYER_NORMAL)
        }
    }

    pub fn add_ui(&self, object: RenderObject) -> u64 {
        self.add_with(object, LAYER_UI)
    }

    pub fn add_with_tag(&self, mut object: RenderObject, layer: u64, tag: TagId) -> u64 {
        object.add_tag(tag);
        self.add_with(object, layer)
    }

    pub fn add_with_tags(&self, mut object: RenderObject, layer: u64, tags: &[TagId]) -> u64 {
        for tag in tags {
            object.add_tag(*tag);
        }
        self.add_with(object, layer)
    }

    pub fn add_with(&self, mut object: RenderObject, layer: u64) -> u64 {
        let id = self.context.alloc_object_id();
        if !object.has_name() {
            object.set_name(&format!("Object {}", id));
        }

        self.storage.insert(id, ObjectWrapper::new(layer, object));
        let mut q = self.queue.lock().unwrap();

        let entry = q.entry(layer);
        let entry = entry.or_insert_with(|| {
            let camera = self.main_camera_ref();
            if layer >= LAYER_UI {
                Arc::new(Mutex::new(UISceneSorter::new()))
            } else if layer > LAYER_TRANSPARENT {
                Arc::new(Mutex::new(MaterialSorter::<DistanceSorter>::new(
                    self.storage.clone(),
                    camera,
                )))
            } else {
                Arc::new(Mutex::new(MaterialSorter::<DistanceSorter>::new(
                    self.storage.clone(),
                    camera,
                )))
            }
        });
        entry.lock().unwrap().add(id);

        id
    }

    pub fn extend(&self, scene: &Scene) {
        scene.clear_inner();

        let store = &scene.storage;
        let keys: Vec<_> = store.iter().map(|k| *k.key()).collect();

        for id in keys {
            let (_, value) = store.remove(&id).unwrap();
            self.add_with(value.object, value.layer);
        }
    }

    fn clear_inner(&self) {
        self.queue.lock().unwrap().clear();
    }

    pub fn remove(&self, id: u64) -> bool {
        if let Some(obj) = self.storage.get(&id) {
            let q = self.queue.lock().unwrap();
            let sorter = q.get(&obj.layer).unwrap();
            sorter.lock().unwrap().remove(id);

            drop(obj);
            self.storage.remove(&id);
            return true;
        }
        false
    }

    pub fn remove_by_tag(&self, tag: TagId) {
        self.remove_if(|v| v.o().has_tag(tag));
    }

    pub fn remove_all(&self) {
        self.remove_if(|v| true);
    }

    pub fn remove_if<F: Fn(&ObjectWrapper) -> bool>(&self, f: F) {
        let mut rm_ids = vec![];
        for v in self.storage.iter() {
            let id = v.key();
            let obj = v.value();
            if f(obj) {
                rm_ids.push(*id);
            }
        }

        for id in rm_ids {
            self.remove(id);
        }
    }

    pub fn modify_if<F: Fn(&mut ObjectWrapper)>(&self, f: F) {
        for mut v in self.storage.iter_mut() {
            let obj = v.value_mut();
            f(obj)
        }
    }

    pub fn get_container(&self) -> SceneStorage {
        self.storage.clone()
    }

    pub fn layers(&self) -> Vec<(u64, Arc<Mutex<dyn Sorter>>)> {
        self.queue
            .lock()
            .unwrap()
            .iter()
            .map(|(a, b)| (*a, b.clone()))
            .collect()
    }

    pub fn material_change(&self) -> bool {
        let mut change = false;
        for s in self.queue.lock().unwrap().values() {
            if s.lock().unwrap().material_change() {
                change = true;
            }
        }
        change
    }

    pub fn clear_objects(&mut self) {
        self.queue.lock().unwrap().clear();
        self.storage.clear();
    }

    pub fn calculate_bytes<'a, I: Iterator<Item = &'a u64>, F: Fn(&RenderObject) -> bool>(
        &self,
        objects: I,
        filter: F,
    ) -> (u64, u64, u64) {
        let mut total_bytes = (0, 0, 0);

        for id in objects {
            let object = self.storage.get(id).unwrap();
            let object = object.o();
            let mesh = object.geometry().mesh();
            let indices = mesh.indices();
            if filter(&object) {
                total_bytes = (
                    total_bytes.0 + indices.len() as u64,
                    total_bytes.1 + mesh.vertices().len() as u64,
                    total_bytes.2 + mesh.vertices_props().len() as u64,
                );
            }
        }
        total_bytes
    }

    pub fn resize(&self, logical: &Size, view_size: &Size) {
        let aspect = view_size.x as f32 / view_size.y as f32;
        // self.ui_camera_ref().make_orthographic();
        let c = self.cameras.lock().unwrap();
        for camera in c.cameras.iter() {
            camera.set_aspect(aspect);
        }
    }
}

pub trait ObjectDrop: std::fmt::Debug {
    fn drop(&self, id: u64);
}

#[derive(Debug, Component)]
pub struct RenderObject {
    geometry: Box<dyn Geometry>,
    material: Arc<Material>,
    z_order: i8,
    visiable: bool,
    name: String,
    tag: HashSet<TagId>,
}

impl RenderObject {
    pub fn new(geometry: Box<dyn Geometry>, material: Arc<Material>) -> Self {
        Self {
            geometry,
            material,
            z_order: 0,
            name: String::default(),
            visiable: true,
            tag: HashSet::default(),
        }
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_owned();
    }

    pub fn has_name(&self) -> bool {
        !self.name.is_empty()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_tag(&mut self, tag: TagId) {
        self.tag.insert(tag);
    }

    pub fn has_tag(&self, tag: TagId) -> bool {
        self.tag.contains(&tag)
    }

    pub fn material(&self) -> &Material {
        self.material.as_ref()
    }

    pub fn material_arc(&self) -> Arc<Material> {
        self.material.clone()
    }

    pub fn is_alpha_test(&self) -> bool {
        self.material.alpha_test().is_some()
    }

    pub fn is_blend(&self) -> bool {
        self.material.blend().is_some()
    }

    pub fn geometry(&self) -> &dyn Geometry {
        self.geometry.as_ref()
    }

    pub fn z_order(&self) -> i8 {
        self.z_order
    }

    pub fn visiable(&self) -> bool {
        self.visiable
    }

    pub fn set_visiable(&mut self, show: bool) {
        self.visiable = show;
    }
}
