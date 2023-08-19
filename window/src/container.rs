use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

pub struct AppResourceContainer {
    res: HashMap<TypeId, Arc<dyn Any>>,
}

impl AppResourceContainer {
    pub fn get<T: 'static>(&self) -> Option<&T> {
        let ty = std::any::TypeId::of::<T>();
        self.res.get(&ty).and_then(|v| v.downcast_ref::<T>())
    }

    pub fn register<T: 'static>(&mut self, t: T) {
        let ty = std::any::TypeId::of::<T>();
        self.res.insert(ty, Arc::new(t));
    }
}
