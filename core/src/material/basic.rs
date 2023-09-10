use std::{fmt::Debug, hash::Hasher, io::Write};

use crate::{
    backends::wgpu_backend::uniform_alignment,
    context::ResourceRef,
    mesh::builder::{InstancePropertyType, MeshPropertyType, INSTANCE_TRANSFORM},
    types::{Color, Vec2f, Vec3f, Vec4f},
    util::any_as_u8_slice,
};

use super::{validate_material_properties, InputResource, MaterialFace};

pub struct BasicMaterialFace {
    pub(crate) variants: Vec<&'static str>,
    pub(crate) variants_name: String,
    pub(crate) texture: InputResource<Color>,
    pub(crate) sampler: Option<ResourceRef>,
    pub(crate) alpha_test: Option<f32>,
    pub(crate) instance: bool,

    uniform: Vec<u8>,

    properties: Vec<MeshPropertyType>,
    instance_properties: Vec<InstancePropertyType>,
}

impl Debug for BasicMaterialFace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BasicMaterialFace")
            .field("variants", &self.variants)
            .field("variants_name", &self.variants_name)
            .field("texture", &self.texture)
            .field("sampler", &self.sampler)
            .finish()
    }
}

impl MaterialFace for BasicMaterialFace {
    fn sort_key(&self) -> u64 {
        let tid = self.texture.sort_key();

        let mut hasher = fxhash::FxHasher64::default();
        hasher.write(self.variants_name.as_bytes());

        let sid = hasher.finish();

        (sid & 0xFFFF_FFFF) | (tid >> 32)
    }
    fn has_alpha_test(&self) -> bool {
        self.alpha_test.is_some()
    }
    fn hash_key(&self) -> u64 {
        let mut h = fxhash::FxHasher::default();
        h.write(self.variants_name.as_bytes());
        h.finish()
    }

    fn material_uniform(&self) -> &[u8] {
        &self.uniform
    }

    fn validate(
        &self,
        t: &crate::mesh::builder::PropertiesFrame<MeshPropertyType>,
        i: Option<&crate::mesh::builder::PropertiesFrame<InstancePropertyType>>,
    ) -> anyhow::Result<()> {
        validate_material_properties(t, i, &self.properties, &self.instance_properties)
    }
}

#[derive(Default, Debug, Clone)]
pub struct BasicMaterialFaceBuilder {
    alpha_test: Option<f32>,
    instance: bool,
    sampler: Option<ResourceRef>,
    texture: InputResource<Color>,
}

impl BasicMaterialFaceBuilder {
    pub fn new() -> Self {
        Self {
            alpha_test: None,
            ..Default::default()
        }
    }
    pub fn instance(mut self) -> Self {
        self.set_instance();
        self
    }

    pub fn set_instance(&mut self) {
        self.instance = true;
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
        self.alpha_test = Some(cut);
    }

    pub fn build(self) -> BasicMaterialFace {
        let mut properties = vec![];
        let mut instance_properties = vec![];
        let mut uniform = vec![];

        if self.instance {
            instance_properties.push(INSTANCE_TRANSFORM);
        }

        let mut variants = vec![];
        for res_ty in self.texture.iter() {
            match res_ty {
                super::InputResourceIterItem::Constant(c) => {
                    variants.push("CONST_COLOR");
                    uniform.write_all(any_as_u8_slice(&Vec3f::new(c.x, c.y, c.z)));
                }
                super::InputResourceIterItem::PreVertex => {
                    properties.push(MeshPropertyType::new::<Color>("color"));
                    variants.push("VERTEX_COLOR");
                }
                super::InputResourceIterItem::Texture(_) => {
                    variants.push("TEXTURE");
                    properties.push(MeshPropertyType::new::<Vec2f>("texture"));
                }
                super::InputResourceIterItem::Instance => {
                    variants.push("INSTANCE");
                    instance_properties.push(InstancePropertyType::new::<Color>("color"));
                    variants.push("CONST_COLOR_INSTANCE");
                }
            }
        }
        if let Some(cut) = self.alpha_test {
            uniform.write_all(any_as_u8_slice(&cut));
        }

        if self.alpha_test.is_some() {
            variants.push("ALPHA_TEST");
        }
        if self.instance {
            variants.push("INSTANCE");
        }
        uniform_alignment(&mut uniform);

        BasicMaterialFace {
            variants_name: tshader::variants_name(&variants[..]),
            variants,
            texture: self.texture,
            sampler: self.sampler,
            alpha_test: self.alpha_test,
            uniform,
            instance: self.instance,
            properties,
            instance_properties,
        }
    }
}
