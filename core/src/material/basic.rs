use std::{
    fmt::Debug,
    hash::Hasher,
    io::Write,
};

use tshader::{VariantFlags, VariantFlagsBuilder};

use crate::{
    context::ResourceRef, material::input::*, mesh::builder::{InstancePropertyType, MeshPropertyType, INSTANCE_TRANSFORM}, render::pso::BindGroupType, types::{Color, Vec2f}
};

use super::{bind::{BindingResourceMap, BindingResourceProvider, ShaderBindingResource}, validate_material_properties, MaterialFace};

pub struct BasicMaterialFace {
    pub(crate) is_instance: bool,
    pub(crate) variants: VariantFlags,

    pub(crate) properties: Vec<MeshPropertyType>,
    pub(crate) instance_properties: Vec<InstancePropertyType>,

    resource: BindingResourceMap,
}

impl Debug for BasicMaterialFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasicMaterialFace")
            .field("variants", &self.variants)
            .finish()
    }
}

impl MaterialFace for BasicMaterialFace {
    fn sort_key(&self) -> u64 {
        let mut hasher = fxhash::FxHasher64::default();
        let tid = if let ShaderBindingResource::Resource(texture) = &self.resource.query_resource("texture") {
            texture.id()
        } else {
            0
        };

        hasher.write(self.variants.key().as_bytes());
        let sid = hasher.finish();

        (sid & 0xFFFF_FFFF) | (tid >> 32)
    }

    fn name(&self) -> &str {
        "basic_material"
    }

    fn variants(&self) -> &VariantFlags {
        &self.variants
    }

    fn validate(
        &self,
        t: &crate::mesh::builder::PropertiesFrame<MeshPropertyType>,
        i: Option<&crate::mesh::builder::PropertiesFrame<InstancePropertyType>>,
    ) -> anyhow::Result<()> {
        validate_material_properties(t, i, &self.properties, &self.instance_properties)
    }
}

impl BindingResourceProvider for BasicMaterialFace {
    fn query_resource(&self, name: &str) -> ShaderBindingResource {
        self.resource.query_resource(name)
    }
    fn bind_group(&self) -> crate::render::pso::BindGroupType {
        self.resource.bind_group()
    }
}

#[derive(Default, Debug, Clone)]
pub struct BasicMaterialFaceBuilder {
    alpha_test: InputAlphaTest,
    texture: InputResource<Color>,
    sampler: Option<ResourceRef>,
    is_instance: bool,
}

impl BasicMaterialFaceBuilder {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }
    pub fn instance(mut self) -> Self {
        self.set_instance();
        self
    }

    pub fn set_instance(&mut self) {
        self.is_instance = true;
    }

    pub fn texture(mut self, texture: InputResource<Color>) -> Self {
        self.set_texture(texture);
        self
    }

    pub fn get_texture(&mut self) -> InputResource<Color> {
        self.texture.clone()
    }

    pub fn set_texture(&mut self, texture: InputResource<Color>) {
        self.texture = texture;
    }

    pub fn has_sampler(&self) -> bool {
        self.sampler.is_some()
    }

    pub fn sampler(mut self, sampler: ResourceRef) -> Self {
        self.set_sampler(sampler);
        self
    }

    pub fn set_sampler(&mut self, sampler: ResourceRef) {
        self.sampler = Some(sampler);
    }

    pub fn alpha_test(mut self, cut: f32) -> Self {
        self.set_alpha_test(cut);
        self
    }

    pub fn set_alpha_test(&mut self, cut: f32) {
        self.alpha_test = InputAlphaTest::make_enabled(cut);
    }

    pub fn build(self) -> BasicMaterialFace {
        let mut properties = vec![];
        let mut instance_properties = vec![];
        let resource = BindingResourceMap::new(BindGroupType::Material);

        if self.is_instance {
            instance_properties.push(INSTANCE_TRANSFORM);
        }

        let mut variants = VariantFlagsBuilder::default();
        for res_ty in self.texture.iter() {
            match res_ty {
                InputResourceIterItem::Constant(c) => {
                    variants.add_flag("CONST_COLOR");
                    resource.upsert("const_color", c);
                }
                InputResourceIterItem::PreVertex => {
                    properties.push(MeshPropertyType::new::<Color>("color"));
                    variants.add_flag("VERTEX_COLOR");
                }
                InputResourceIterItem::Texture(t) => {
                    variants.add_flag("TEXTURE");
                    properties.push(MeshPropertyType::new::<Vec2f>("texture"));
                    resource.upsert("const_color", t.clone());
                }
                InputResourceIterItem::Instance => {
                    // variants.add_flag("INSTANCE");
                    instance_properties.push(InstancePropertyType::new::<Color>("color"));
                    variants.add_flag("CONST_COLOR_INSTANCE");
                }
            }
        }
        if self.alpha_test.is_enable() {
            variants.add_flag("ALPHA_TEST");
            resource.upsert("alpha_test", self.alpha_test.cutoff().unwrap());
        }
        if self.is_instance {
            variants.add_flag("INSTANCE");
        }

        let face = BasicMaterialFace {
            variants: variants.build(),
            is_instance: self.is_instance,
            properties,
            instance_properties,
            resource,
        };

        face
    }
}
