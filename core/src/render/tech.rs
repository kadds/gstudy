use std::sync::Arc;

use dashmap::DashMap;
use tshader::{ShaderTech, ShaderTechLoader, VariantFlags};

use crate::material::Material;

use super::pso::{PipelineStateObject, PipelineStateObjectCache, RenderDescriptorObject};

use anyhow::Result;

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
struct Key {
    variant_key: u64,
    name: String,
}

pub struct ShaderTechCollection {
    loader: Box<dyn ShaderTechLoader>,
    pso_cache: Box<dyn PipelineStateObjectCache>,
    tech_cache: DashMap<Key, Arc<ShaderTech>>,
}


impl ShaderTechCollection {
    pub fn new(
        loader: Box<dyn ShaderTechLoader>,
        pso_cache: Box<dyn PipelineStateObjectCache>,
    ) -> Self {
        Self {
            loader,
            pso_cache,
            tech_cache: DashMap::new(),
        }
    }

    pub fn get(&self, tech_name: &str, variants: &VariantFlags, instance_id: u64, pass_name: &str) -> Arc<PipelineStateObject> {
        let key = Key {
            variant_key: variants.hash_key(),
            name: tech_name.to_string(),
        };

        let tech = self.tech_cache.get(&key).unwrap();
        let pass = tech.get_pass(pass_name).unwrap();
        let pso = self.pso_cache.get(instance_id, pass);
        pso
    }

    pub fn setup<F: Fn(&str) -> RenderDescriptorObject>(
        &self,
        device: &wgpu::Device,
        tech_name: &str,
        variants: &VariantFlags,
        instance_id: u64,
        get_rdo_fn: F,
    ) -> Result<()> {
        let key = Key {
            variant_key: variants.hash_key(),
            name: tech_name.to_string(),
        };

        if !self.tech_cache.contains_key(&key) {
            let tech = self
                .loader
                .load(device, tech_name, variants)?;

            self.tech_cache.insert(key.clone(), tech);
        }

        let t = self.tech_cache.get(&key).unwrap().clone();
        for pass in &t.pass {
            self.pso_cache
                .load(device, instance_id, pass.clone(), get_rdo_fn(&pass.name));
        }

        Ok(())
    }

    pub fn setup_materials<'b, F: Fn(&Material, &str) -> RenderDescriptorObject>(
        &self,
        device: &wgpu::Device,
        materials: &[Arc<Material>],
        tech_name: &str,
        get_rdo_fn: F,
    ) -> Result<()> {
        for material in materials {
            self.setup(device, tech_name, material.face().variants(),  material.id().id(), |pass_name| get_rdo_fn(&material, pass_name))?;
        }
        Ok(())
    }
}
