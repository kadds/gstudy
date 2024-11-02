use std::sync::Arc;

use dashmap::DashMap;

use crate::{tech::ShaderTech, ShaderTechLoader, VariantFlags};

#[derive(Debug, Hash, PartialEq, Eq)]
struct CacheKey {
    name: String,
    variant_key: String,
}

pub struct ShaderCache {
    loader: Box<dyn ShaderTechLoader>,
    active: DashMap<CacheKey, Arc<ShaderTech>>, 
}

impl ShaderCache {
    pub fn new(loader: Box<dyn ShaderTechLoader>) -> Self {
       Self {
        loader,
        active: DashMap::new(),
       } 
    }

    pub fn get(&self, device: &wgpu::Device, name: &str, variant: &VariantFlags) -> anyhow::Result<Arc<ShaderTech>> {
        let key = CacheKey {
            name: name.to_owned(),
            variant_key: variant.key().to_owned(),
        };

        if let Some(v) = self.active.get(&key) {
            return Ok(v.value().clone());
        }
        let tech = self.loader.load(device, name.to_owned(), variant)?;
        self.active.insert(key, tech.clone());
        Ok(tech)
    }

    pub fn preload(&self) {

    }
}