use bevy_ecs::prelude::Component;

use crate::{
    context::RContextRef,
    geometry::Geometry,
    material::{Material, MaterialId},
};
use std::{
    any::{Any, TypeId},
    collections::{BTreeMap, HashMap, HashSet},
    sync::{atomic::AtomicBool, Arc},
};

use super::Camera;

#[derive(Debug, Default)]
pub struct LayerObjects {
    pub map: HashMap<MaterialId, smallvec::SmallVec<[u64; 2]>>, // material id -> objects
    dirty: bool,

    pub sorted_objects: BTreeMap<u64, Arc<Material>>, // sort key -> material id
}

pub const LAYER_BACKGROUND: u64 = 10000;
pub const LAYER_TRANSPARENT: u64 = 20000;
pub const LAYER_ALPHA_TEST: u64 = 30000;
pub const LAYER_NORMAL: u64 = 4000;
pub const LAYER_UI: u64 = 10_0000;

#[derive(Debug)]
pub struct Scene {
    context: RContextRef,

    objects: HashMap<u64, RenderObject>,

    // reader layer -> objects
    layers: BTreeMap<u64, LayerObjects>,

    drop_objects: Vec<u64>,

    cameras: Vec<Arc<Camera>>,
    ui_camera: Option<Arc<Camera>>,

    material_face_map: HashMap<TypeId, i32>,
    material_face_change_map: HashMap<TypeId, i32>,
}

impl Scene {
    pub fn new(context: RContextRef) -> Self {
        Self {
            context,
            objects: HashMap::new(),
            layers: BTreeMap::new(),
            drop_objects: Vec::new(),

            cameras: Vec::new(),
            ui_camera: None,
            material_face_map: HashMap::new(),
            material_face_change_map: HashMap::new(),
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
        self.objects.len()
    }

    pub fn add_object(&mut self, object: RenderObject) -> u64 {
        if object.is_alpha_test() {
            self.add_object_with(object, LAYER_ALPHA_TEST)
        } else if object.is_blend() {
            self.add_object_with(object, LAYER_TRANSPARENT)
        } else {
            self.add_object_with(object, LAYER_NORMAL)
        }
    }

    pub fn add_ui(&mut self, object: RenderObject) -> u64 {
        self.add_object_with(object, LAYER_UI)
    }

    pub fn add_object_with(&mut self, object: RenderObject, layer: u64) -> u64 {
        let id = self.context.alloc_object_id();

        let material = object.material();
        let material_id = material.id();

        let entry = self.layers.entry(layer);
        let entry = entry.or_insert_with(|| LayerObjects::default());

        entry.map.entry(material_id).or_default().push(id);
        entry.dirty = true;
        self.material_face_change_map
            .entry(material.face_id())
            .and_modify(|v| *v += 1)
            .or_insert(1);

        self.objects.insert(id, object);

        id
    }

    // pub fn delete_object(&mut self, id: u64) -> bool {
    //     let object = match self.objects.get(&id) {
    //         Some(v) => v,
    //         None => return false,
    //     };
    //     let material_id = object.material().id();

    //     self.objects.remove(&id);

    //     if let Some(set) = self.material_objects.get_mut(&typeid) {
    //         set.remove(&id);
    //     }
    //     true
    // }

    pub fn get_object(&self, id: u64) -> Option<&RenderObject> {
        self.objects.get(&id)
    }

    pub fn get_object_mut(&mut self, id: u64) -> Option<&mut RenderObject> {
        self.objects.get_mut(&id)
    }

    pub fn layers(&self) -> impl Iterator<Item = (&u64, &LayerObjects)> {
        self.layers.iter()
    }

    pub fn material_change(&mut self) -> bool {
        let m = self.material_face_map.clone();

        for (id, n) in &self.material_face_change_map {
            let entry = self.material_face_map.entry(*id);
            entry.or_default();
            *self.material_face_map.get_mut(id).unwrap() += *n;
        }

        let mut removal = vec![];
        for (id, n) in &self.material_face_map {
            if *n <= 0 {
                removal.push(*id);
            }
        }

        for id in removal {
            self.material_face_map.remove(&id);
        }

        self.material_face_change_map.clear();

        if m.len() != self.material_face_map.len() {
            return true;
        }

        for (key, _) in m {
            if !self.material_face_map.contains_key(&key) {
                return true;
            }
        }
        // log::info!("key {:?} {}", self.material_face_map, self.object_size());
        false
    }

    pub fn layer(&self, layer: u64) -> &LayerObjects {
        self.layers.get(&layer).as_ref().unwrap()
    }

    pub fn sort_all<S: FnMut(u64, &Material) -> u64>(&mut self, mut sorter: S) {
        for (level, objects) in self.layers.iter_mut() {
            if !objects.dirty {
                continue;
            }

            objects.sorted_objects.clear();

            for (mat_id, id) in &objects.map {
                let first = id.iter().next().unwrap();
                let first_obj = self.objects.get(first).unwrap();

                let material = first_obj.material.clone();
                let key = sorter(*level, &material);

                objects.sorted_objects.insert(key, material);
            }

            objects.dirty = false;
        }
    }

    pub fn update(&self, delta: f64) {}

    pub fn clear_objects(&mut self) {
        for (_, layer) in &mut self.layers {
            layer.sorted_objects.clear();
            layer.map.clear();
            layer.dirty = true;
            self.material_face_change_map.clear();
            self.material_face_map.clear();
        }
        self.objects.clear();
    }

    pub fn clear_layer_objects(&mut self, layer: u64) {
        let v = self.layers.get_mut(&layer);
        if let Some(v) = v {
            for (_, objs) in &v.map {
                for obj in objs {
                    let material = self.objects.get(&obj).unwrap().material();
                    self.material_face_change_map
                        .entry(material.face_id())
                        .and_modify(|v| *v -= 1)
                        .or_insert(-1);
                    self.objects.remove(&obj);
                }
            }
            v.map.clear();
            v.sorted_objects.clear();
            v.dirty = true;
        }
    }

    pub fn drop_objects(&self) -> &[u64] {
        &self.drop_objects
    }

    pub fn calculate_bytes<'a, I: Iterator<Item = &'a u64>, F: Fn(&RenderObject) -> bool>(
        &self,
        objects: I,
        filter: F,
    ) -> (u64, u64, u64) {
        let mut total_bytes = (0, 0, 0);

        for id in objects {
            let object = self.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let indices = mesh.indices();
            if filter(object) {
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
}

impl RenderObject {
    pub fn new(geometry: Box<dyn Geometry>, material: Arc<Material>) -> Self {
        Self {
            geometry,
            material,
            z_order: 0,
        }
    }

    pub fn material(&self) -> &Material {
        self.material.as_ref()
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

    pub fn sub_objects(&self) -> usize {
        0
    }

    pub fn z_order(&self) -> i8 {
        self.z_order
    }
}

fn mesh_render_system() {}
