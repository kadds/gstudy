use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub struct Container {
    m: Mutex<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl Container {
    pub fn register<T: 'static + Send + Sync>(&self, t: T) {
        let mut m = self.m.lock().unwrap();
        m.insert(t.type_id(), Arc::new(t));
    }

    pub fn register_arc<T: 'static + Send + Sync>(&self, t: Arc<T>) {
        let mut m = self.m.lock().unwrap();
        m.insert((&*t).type_id(), t);
    }

    pub fn get<T: 'static + Send + Sync>(&self) -> Option<Arc<T>> {
        let m = self.m.lock().unwrap();
        m.get(&std::any::TypeId::of::<T>())
            .and_then(|v| v.clone().downcast::<T>().ok())
    }
}

pub struct LockResource<T> {
    data: Mutex<T>,
}

impl<T> LockResource<T>
where
    T: Clone,
{
    pub fn new(t: T) -> Self {
        Self {
            data: Mutex::new(t),
        }
    }

    pub fn get(&self) -> T {
        self.data.lock().unwrap().clone()
    }

    pub fn set(&self, t: T) {
        *self.data.lock().unwrap() = t;
    }
}
