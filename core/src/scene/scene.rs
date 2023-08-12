use bevy_ecs::prelude::*;
use bytes::Bytes;
use dashmap::DashMap;

use crate::{
    context::RContextRef,
    geometry::Geometry,
    material::{Material, MaterialId},
    util::StringIdAllocMap,
};
use std::{
    any::TypeId,
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Debug,
    rc::Rc,
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

pub type TagId = u32;
pub type SceneStorage = Arc<DashMap<u64, ObjectWrapper>>;
pub const INVALID_TAG_ID: TagId = 0;

pub struct Scene {
    context: RContextRef,

    storage: SceneStorage,

    // reader layer -> objects
    queue: Mutex<BTreeMap<u64, Arc<Mutex<dyn Sorter>>>>,

    cameras: Vec<Arc<Camera>>,
    ui_camera: Option<Arc<Camera>>,

    tags: StringIdAllocMap<TagId>,
}

impl std::fmt::Debug for Scene {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scene")
            .field("context", &self.context)
            .field("objects", &self.storage)
            .field("cameras", &self.cameras)
            .field("ui_camera", &self.ui_camera)
            .finish()
    }
}

impl Scene {
    pub fn new(context: RContextRef) -> Self {
        Self {
            context,
            storage: SceneStorage::new(DashMap::new()),

            queue: Mutex::new(BTreeMap::new()),

            cameras: Vec::new(),
            ui_camera: None,

            tags: StringIdAllocMap::new_with_begin(1),
        }
    }

    pub fn set_main_camera(&mut self, camera: Arc<Camera>) {
        if self.cameras.is_empty() {
            self.cameras.push(camera);
        } else {
            self.cameras[0] = camera;
        }
    }

    pub fn main_camera(&self) -> Option<&Camera> {
        self.cameras.get(0).map(|v| v.as_ref())
    }

    pub fn main_camera_ref(&self) -> Arc<Camera> {
        self.cameras[0].clone()
    }

    pub fn set_ui_camera(&mut self, camera: Arc<Camera>) {
        self.ui_camera = Some(camera);
    }

    pub fn ui_camera(&self) -> Option<&Camera> {
        self.ui_camera.as_ref().map(|v| v.as_ref())
    }

    pub fn object_size(&self) -> usize {
        self.storage.len()
    }

    pub fn new_tag(&mut self, name: &str) -> TagId {
        let id = self.tags.alloc_or_get(name);
        id
    }

    pub fn delete_tag(&mut self, id: TagId) {
        self.tags.dealloc(id);
    }

    pub fn delete_tag_by_name(&mut self, name: &str) {
        if let Some(id) = self.tags.get_by_name(name) {
            self.tags.dealloc(id);
        }
    }

    pub fn add(&mut self, object: RenderObject) -> u64 {
        if object.is_alpha_test() {
            self.add_with(object, LAYER_ALPHA_TEST)
        } else if object.is_blend() {
            self.add_with(object, LAYER_TRANSPARENT)
        } else {
            self.add_with(object, LAYER_NORMAL)
        }
    }

    pub fn add_ui(&mut self, object: RenderObject) -> u64 {
        self.add_with(object, LAYER_UI)
    }

    pub fn add_with_tag_id(&mut self, mut object: RenderObject, layer: u64, tag_id: TagId) -> u64 {
        object.add_tag(tag_id);
        self.add_with(object, layer)
    }

    pub fn add_with_tag_ids(
        &mut self,
        mut object: RenderObject,
        layer: u64,
        tag_id: &[TagId],
    ) -> u64 {
        for id in tag_id {
            object.add_tag(*id);
        }
        self.add_with(object, layer)
    }

    pub fn add_with(&mut self, mut object: RenderObject, layer: u64) -> u64 {
        let id = self.context.alloc_object_id();
        if !object.has_name() {
            object.set_name(&format!("Object {}", id));
        }

        self.storage.insert(id, ObjectWrapper::new(layer, object));
        let mut q = self.queue.lock().unwrap();

        let entry = q.entry(layer);
        let entry = entry.or_insert_with(|| {
            if layer >= LAYER_UI {
                Arc::new(Mutex::new(UISceneSorter::new()))
            } else if layer > LAYER_TRANSPARENT {
                Arc::new(Mutex::new(MaterialSorter::<DistanceSorter>::new(
                    self.storage.clone(),
                )))
            } else {
                Arc::new(Mutex::new(MaterialSorter::<DistanceSorter>::new(
                    self.storage.clone(),
                )))
            }
        });
        entry.lock().unwrap().add(id);

        id
    }

    pub fn remove(&mut self, id: u64) -> bool {
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

    pub fn remove_by_tag(&mut self, tag: TagId) {
        self.remove_if(|v| v.o().has_tag(tag));
    }

    pub fn remove_if<F: Fn(&ObjectWrapper) -> bool>(&mut self, f: F) {
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

    pub fn material_change(&mut self) -> bool {
        let mut change = false;
        for s in self.queue.lock().unwrap().values() {
            if s.lock().unwrap().material_change() {
                change = true;
            }
        }
        change
    }

    // pub fn layer(&self, layer: u64) -> &dyn Sorter {
    //     self.layers.get(&layer).as_ref().unwrap().borrow()
    // }

    pub fn update(&self, delta: f64) {}

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
