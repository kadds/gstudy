use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use ordered_float::OrderedFloat;

use crate::{material::MaterialId, types::Bound};

use super::{Camera, SceneStorage, UNKNOWN_OBJECT};

pub trait Sorter: Send + Sync {
    fn set_camera(&mut self, camera: Arc<Camera>);
    fn add(&mut self, object: u64);
    fn sort_and_cull(&mut self) -> Vec<u64>;
    fn remove(&mut self, object: u64);
    fn material_change(&mut self) -> bool;
}

pub trait SorterFactory {
    fn create(objects: SceneStorage) -> Self;
}

pub struct UISceneSorter {
    objects: Vec<u64>,
    object_position: HashMap<u64, usize>,
    new_material: bool,
}

impl Sorter for UISceneSorter {
    fn add(&mut self, object: u64) {
        let pos = self.objects.len();
        if pos == 0 {
            self.new_material = true;
        }
        self.objects.push(object);
        self.object_position.insert(object, pos);
    }

    fn sort_and_cull(&mut self) -> Vec<u64> {
        let res: Vec<_> = self
            .objects
            .iter()
            .cloned()
            .filter(|v| *v != UNKNOWN_OBJECT)
            .collect();
        self.object_position.clear();
        for (idx, obj) in res.iter().enumerate() {
            self.object_position.insert(*obj, idx);
        }
        self.objects = res.clone();

        res
    }

    fn remove(&mut self, object: u64) {
        if let Some(pos) = self.object_position.get(&object) {
            self.objects[*pos] = UNKNOWN_OBJECT;
        }
        self.object_position.remove(&object);
    }

    fn set_camera(&mut self, _camera: Arc<Camera>) {}

    fn material_change(&mut self) -> bool {
        if self.new_material {
            self.new_material = false;
            return true;
        }
        false
    }
}

impl UISceneSorter {
    pub fn new() -> Self {
        Self {
            object_position: HashMap::new(),
            objects: Vec::new(),
            new_material: false,
        }
    }
}

pub struct DistanceSorter {
    objects: HashSet<u64>,
    storage: SceneStorage,
    camera: Option<Arc<Camera>>,
}

impl Sorter for DistanceSorter {
    fn set_camera(&mut self, camera: Arc<Camera>) {
        self.camera = Some(camera);
    }

    fn add(&mut self, object: u64) {
        self.objects.insert(object);
    }

    fn sort_and_cull(&mut self) -> Vec<u64> {
        if let Some(c) = &self.camera {
            let camera_pos = c.from();
            // cull first
            let frustum = c.frustum_worldspace();

            let mut res: Vec<_> = self
                .objects
                .iter()
                .cloned()
                .filter_map(|v| {
                    let o = self.storage.get(&v).unwrap();
                    let o = o.o();
                    if !o.visiable() {
                        None
                    } else {
                        if let Some(aabb) = o.geometry().aabb() {
                            if !aabb.in_frustum(&frustum) {
                                return None;
                            }
                        }
                        Some((
                            o.geometry().aabb().map_or_else(
                                || OrderedFloat(0f32),
                                |aabb| {
                                    let a = aabb.center().into();
                                    let b = camera_pos.into();
                                    OrderedFloat::<f32>(nalgebra::distance_squared(&a, &b))
                                },
                            ),
                            v,
                        ))
                    }
                })
                .collect();
            res.sort_by(|a, b| b.0.cmp(&a.0));

            return res.iter().map(|v| v.1).collect();
        }
        self.objects.iter().cloned().collect()
    }

    fn remove(&mut self, object: u64) {
        self.objects.remove(&object);
    }

    fn material_change(&mut self) -> bool {
        false
    }
}

impl SorterFactory for DistanceSorter {
    fn create(objects: SceneStorage) -> Self {
        Self {
            objects: HashSet::new(),
            storage: objects,
            camera: None,
        }
    }
}

pub struct MaterialSorter<T> {
    map: HashMap<MaterialId, (T, u64)>,
    storage: SceneStorage,
    materials: HashMap<TypeId, u64>,
    new_material: bool,
    camera: Option<Arc<Camera>>,
}

impl<T> MaterialSorter<T> {
    pub fn new(storage: SceneStorage, camera: Option<Arc<Camera>>) -> Self {
        Self {
            map: HashMap::new(),
            storage,
            materials: HashMap::new(),
            new_material: false,
            camera,
        }
    }
}

impl<T> Sorter for MaterialSorter<T>
where
    T: Sorter + SorterFactory,
{
    fn set_camera(&mut self, camera: Arc<Camera>) {
        self.camera = Some(camera.clone());
        for t in &mut self.map.values_mut() {
            t.0.set_camera(camera.clone());
        }
    }

    fn add(&mut self, object: u64) {
        let obj = self.storage.get(&object).unwrap();
        let obj = obj.o();

        let material = obj.material();
        let material_id = material.id();
        let face_id = material.face_id();

        let t = self.map.entry(material_id).or_insert_with(|| {
            let mut t = T::create(self.storage.clone());
            if let Some(c) = &self.camera {
                t.set_camera(c.clone());
            }
            (t, material.face().sort_key())
        });
        t.0.add(object);

        let v = self
            .materials
            .entry(face_id)
            .and_modify(|v| *v += 1)
            .or_insert(1);
        if *v == 1 {
            self.new_material = true;
        }
    }

    fn sort_and_cull(&mut self) -> Vec<u64> {
        let mut res = vec![];
        let mut material_list = vec![];
        material_list.reserve(self.map.len());

        for (material_id, (t, sort_key)) in &self.map {
            material_list.push((*sort_key, *material_id));
        }
        material_list.sort_by(|a, b| a.0.cmp(&b.0));

        for (_, material_id) in material_list {
            let t = self.map.get_mut(&material_id).unwrap();
            let res2 = t.0.sort_and_cull();
            res.extend(res2);
        }

        res
    }

    fn remove(&mut self, object: u64) {
        if let Some(obj) = self.storage.get(&object) {
            let material = obj.o().material();
            let material_id = material.id();
            let face_id = material.face_id();

            self.materials.entry(face_id).and_modify(|v| *v -= 1);

            self.map
                .entry(material_id)
                .and_modify(|v| v.0.remove(object));
        }
    }

    fn material_change(&mut self) -> bool {
        if self.new_material {
            self.new_material = false;
            return true;
        }
        false
    }
}
