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

    objects: HashMap<u64, Object>,

    // reader layer -> objects
    layers: BTreeMap<u64, LayerObjects>,

    drop_objects: Vec<u64>,

    cameras: Vec<Arc<Camera>>,
    ui_camera: Option<Arc<Camera>>,

    change: AtomicBool,
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
            change: AtomicBool::new(false),
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

    pub fn add_object(&mut self, object: Object) -> u64 {
        if object.is_alpha_test() {
            self.add_object_with(object, LAYER_ALPHA_TEST)
        } else if object.is_blend() {
            self.add_object_with(object, LAYER_TRANSPARENT)
        } else {
            self.add_object_with(object, LAYER_NORMAL)
        }
    }

    pub fn add_ui(&mut self, object: Object) -> u64 {
        self.add_object_with(object, LAYER_UI)
    }

    pub fn add_object_with(&mut self, object: Object, layer: u64) -> u64 {
        let id = self.context.alloc_object_id();

        let material = object.material();
        let material_id = material.id();

        let entry = self.layers.entry(layer);
        let entry = entry.or_insert_with(|| LayerObjects::default());

        entry.map.entry(material_id).or_default().push(id);
        entry.dirty = true;

        self.objects.insert(id, object);

        self.change.store(true, atomic::Ordering::SeqCst);

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

    pub fn get_object(&self, id: u64) -> Option<&Object> {
        self.objects.get(&id)
    }

    pub fn get_object_mut(&mut self, id: u64) -> Option<&mut Object> {
        self.objects.get_mut(&id)
    }

    pub fn layers(&self) -> impl Iterator<Item = (&u64, &LayerObjects)> {
        self.layers.iter()
    }

    pub fn change(&self) -> bool {
        let change = self.change.load(atomic::Ordering::SeqCst);
        change
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

    pub fn update(&self, delta: f64) {
        self.change.store(false, atomic::Ordering::SeqCst)
    }

    pub fn clear_objects(&mut self) {
        for (_, layer) in &mut self.layers {
            layer.sorted_objects.clear();
            layer.map.clear();
            layer.dirty = true;
        }
        self.change.store(true, atomic::Ordering::SeqCst);
        self.objects.clear();
    }

    pub fn clear_layer_objects(&mut self, layer: u64) {
        let v = self.layers.get_mut(&layer);
        if let Some(v) = v {
            for (_, objs) in &v.map {
                for obj in objs {
                    self.objects.remove(&obj);
                }
            }
            self.change.store(true, atomic::Ordering::SeqCst);
            v.map.clear();
            v.sorted_objects.clear();
            v.dirty = true;
        }
    }

    pub fn drop_objects(&self) -> &[u64] {
        &self.drop_objects
    }
}

pub trait ObjectDrop: std::fmt::Debug {
    fn drop(&self, id: u64);
}

#[derive(Debug)]
pub struct Object {
    geometry: Box<dyn Geometry>,
    material: Arc<Material>,
    z_order: i8,
}

impl Object {
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
