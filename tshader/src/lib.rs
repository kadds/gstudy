use std::{
    hash::Hasher, sync::Arc
};

use itertools::Itertools;

#[derive(Debug)]
pub struct VariantFlags {
    full_key: String,
    view: Vec<String>,
    hash_key: u64,
}

impl Default for VariantFlags {
    fn default() -> Self {
        Self {
            full_key: "-".to_owned(),
            view: vec![],
            hash_key: 123,
        }
    }
}

impl VariantFlags {
    pub fn new(view: Vec<String> ) -> Self {
        let full_key = view.clone().into_iter().join("+");

        let mut h = fxhash::FxHasher::default();
        h.write(full_key.as_bytes());
        let hash_key = h.finish();
        Self {
            full_key,
            view,
            hash_key,
        }
    }
    pub fn key(&self) -> &str {
        &self.full_key
    }

    pub fn hash_key(&self) -> u64 {
        self.hash_key
    }
}

#[derive(Debug, Default)]
pub struct VariantFlagsBuilder {
    view: Vec<String>,
}

impl VariantFlagsBuilder {
    pub fn add_flag(&mut self, s: &str) {
        self.view.push(s.to_owned());
    }
    pub fn build(self) -> VariantFlags {
        VariantFlags::new(self.view)
    }
}

pub mod tech;

pub trait ShaderTechLoader {
    fn load(
        &self,
        device: &wgpu::Device,
        name: &str,
        variant: &VariantFlags,
    ) -> anyhow::Result<Arc<tech::ShaderTech>>;
}

pub mod default_loader;
pub mod reflection;

pub use tech::Pass;
pub use tech::Shader;
pub use tech::ShaderTech;

// pub fn default_shader_cache() -> ShaderCache {
//     ShaderCache::new(Box::new(
//         default_loader::DefaultShaderTechLoader::new("./shaders/".into()).unwrap(),
//     ))
// }

pub fn default_shader_tech_loader() -> Box<dyn ShaderTechLoader> {
    Box::new(default_loader::DefaultShaderTechLoader::new("./shaders/".into()).unwrap())
}
