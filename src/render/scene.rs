use crate::{geometry::Geometry, render::Material};
use std::{
    any::{TypeId, Any},
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Debug, Default)]
pub struct MaterialGroup {
    pub map: HashMap<u64, Vec<u64>>,
}

impl MaterialGroup {
    pub fn get_objects_from_material(&self, id: u64) -> &Vec<u64> {
        self.map.get(&id).unwrap()
    }
}


#[derive(Debug)]
pub struct Scene {
    last_id: u64,
    dyn_material_map: HashMap<u64, Arc<dyn Material>>,
    objects: HashMap<u64, Object>,

    material_objects: HashMap<TypeId, MaterialGroup>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            last_id: 1,
            dyn_material_map: HashMap::new(),
            objects: HashMap::new(),
            material_objects: HashMap::new(),
        }
    }

    pub fn add_object(&mut self, object: Object) -> u64 {
        let id = self.last_id;
        self.last_id+=1;


        // save material
        let material_id = object.material().material_id();
        let material_id = if let Some(material_id) = material_id {
            if let Some(material) = self.dyn_material_map.get(&material_id) {
                if Arc::as_ptr(material) != Arc::as_ptr(&object.material) {
                    panic!("Do not use material between scenes")
                }
            }
            material_id
        } else {
            let material_id = self.last_id;
            object.material.reset_material_id(Some(material_id));
            self.last_id+=1;
            self.dyn_material_map.insert(material_id, object.material.clone());
            material_id
        };

        // insert into material group
        let type_id = object.material.as_ref().type_id();

        let entry = self.material_objects.entry(type_id);
        entry.or_insert_with(|| MaterialGroup::default()).map.entry(material_id).or_default().push(id);
        self.objects.insert(id, object);

        id
    }

    pub fn delete_object(&mut self, id: u64) -> bool {
        // let object = match self.objects.get(&id) {
        //     Some(v) => v,
        //     None => return false,
        // };
        // let typeid = object.material().type_id();
        // self.objects.remove(&id);

        // if let Some(set) = self.material_objects.get_mut(&typeid) {
        //     set.remove(&id);
        // }
        true
    }

    pub fn get_object(&self, id: u64) -> Option<&Object> {
        self.objects.get(&id)
    }


    pub fn get_material(&self, id: u64) -> Option<Arc<dyn Material>> {
        self.dyn_material_map.get(&id).cloned()
    }

    pub fn load_material_objects(&self) -> &HashMap<TypeId, MaterialGroup> {
        &self.material_objects
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
