use core::{context::ResourceRef, material::{bind::{BindingResourceMap, BindingResourceProvider, ShaderBindingResource}, MaterialFace}, render::pso::BindGroupType};
use std::hash::Hasher;

use tshader::VariantFlagsBuilder;

#[derive(Debug)]
pub struct EguiMaterialFace {
    variants: tshader::VariantFlags,

    resource: BindingResourceMap,
}

impl MaterialFace for EguiMaterialFace {
    fn sort_key(&self) -> u64 {
        if let ShaderBindingResource::Resource(texture) = &self.resource.query_resource("texture") {
            let mut hasher = fxhash::FxHasher64::default();
            hasher.write_u64(texture.id());
            return hasher.finish();
        }
        0
    }

    fn name(&self) -> &str {
        "egui"
    }

    fn validate(
            &self,
            t: &core::mesh::builder::PropertiesFrame<core::mesh::builder::MeshPropertyType>,
            i: Option<&core::mesh::builder::PropertiesFrame<core::mesh::builder::InstancePropertyType>>,
        ) -> anyhow::Result<()> {
        Ok(())
    }

    fn variants(&self) -> &tshader::VariantFlags {
        &self.variants
    }
}

impl BindingResourceProvider for EguiMaterialFace {
    fn query_resource(&self, name: &str) -> ShaderBindingResource {
        self.resource.query_resource(name)
    }

    fn bind_group(&self) -> core::render::pso::BindGroupType {
        self.resource.bind_group()
    }
}

#[derive(Debug, Default)]
pub struct EguiMaterialFaceBuilder {
    texture: Option<ResourceRef>,
    sampler: Option<ResourceRef>
}

impl EguiMaterialFaceBuilder {
    pub fn with_texture(mut self, texture: ResourceRef) -> Self {
        self.texture = Some(texture);
        self
    }
    pub fn with_sampler(mut self, sampler: ResourceRef) -> Self {
        self.sampler = Some(sampler);
        self
    }
    pub fn build(self) -> EguiMaterialFace {
        let resource = BindingResourceMap::new(BindGroupType::Material);
        resource.upsert("texture", self.texture.unwrap());
        resource.upsert("sampler", self.sampler.unwrap());

        EguiMaterialFace {
            variants: VariantFlagsBuilder::default().build(),
            resource,
        }
    }
}
