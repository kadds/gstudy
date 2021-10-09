use crate::{geometry::Geometry, render::Material};
use std::{
    any::TypeId,
    collections::{BTreeMap, HashMap, HashSet},
    sync::{Arc, Mutex},
};

#[derive(Debug)]
pub struct Scene {
    last_id: u64,
    objects: HashMap<u64, Object>,
    material_objects: HashMap<TypeId, HashSet<u64>>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            last_id: 0,
            objects: HashMap::new(),
            material_objects: HashMap::new(),
        }
    }

    pub fn add_object(&mut self, object: Object) -> u64 {
        let id = self.last_id;
        let typeid = object.material().type_id();
        if !self.material_objects.contains_key(&typeid) {
            self.material_objects.insert(typeid.clone(), HashSet::new());
        }
        self.material_objects.get_mut(&typeid).unwrap().insert(id);
        self.objects.insert(id, object);
        self.last_id += 1;
        id
    }

    pub fn delete_object(&mut self, id: u64) -> bool {
        let object = match self.objects.get(&id) {
            Some(v) => v,
            None => return false,
        };
        let typeid = object.material().type_id();
        self.objects.remove(&id);

        if let Some(set) = self.material_objects.get_mut(&typeid) {
            set.remove(&id);
        }
        true
    }

    pub fn get_object(&self, id: u64) -> Option<&Object> {
        self.objects.get(&id)
    }

    pub fn load_material_objects(&self) -> &HashMap<TypeId, HashSet<u64>> {
        &self.material_objects
    }

    pub fn prepare_frame(&self) {
        // let mut inner = self.inner.lock().unwrap();
        // for (key, value) in inner.objects {
        //     // self.objects.insert()
        // }
    }
}

#[derive(Debug)]
pub struct Object {
    geometry: Box<dyn Geometry>,
    material: Arc<dyn Material>,
}

impl Object {
    pub fn new(geometry: Box<dyn Geometry>, material: Arc<dyn Material>) -> Self {
        Self { geometry, material }
    }

    pub fn material(&self) -> &dyn Material {
        self.material.as_ref()
    }

    pub fn geometry(&self) -> &dyn Geometry {
        self.geometry.as_ref()
    }
}
