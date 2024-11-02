use core::{
    backends::wgpu_backend::uniform_alignment, context::ResourceRef, material::{bind::{BindingResourceMap, BindingResourceProvider, ShaderBindingResource}, input::{InputResource, InputResourceIterItem}, MaterialFace}, render::pso::BindGroupType, types::{Color, Vec3f, Vec4f}, util::any_as_u8_slice
};
use std::{hash::Hasher, io::Write, panic::panic_any};

use tshader::{VariantFlags, VariantFlagsBuilder};

#[derive(Debug)]
pub struct PhongMaterialFace {
    pub variants_base: VariantFlags,
    pub variants_add: VariantFlags,

    resource: BindingResourceMap,
}

impl MaterialFace for PhongMaterialFace {
    fn sort_key(&self) -> u64 {
        let mut hasher = fxhash::FxHasher64::default();
        hasher.write(self.variants_base.key().as_bytes());

        let sid = hasher.finish();
        let mut hasher2 = fxhash::FxHasher64::default();
        if let ShaderBindingResource::Resource(res) = self.resource.query_resource("diffuse_texture") {
            hasher2.write_u64(res.id());
        }
        if let ShaderBindingResource::Resource(res) = self.resource.query_resource("specular_texture") {
            hasher2.write_u64(res.id());
        }
        if let ShaderBindingResource::Resource(res) = self.resource.query_resource("emissive_texture") {
            hasher2.write_u64(res.id());
        }

        (sid & 0xFFFF_FFFF) | (hasher2.finish() >> 32)
    }
 
    fn name(&self) -> &str {
        "phong"
    }
     
    fn variants(&self) -> &tshader::VariantFlags {
        &self.variants_base
    } 
 
}

impl BindingResourceProvider for PhongMaterialFace {
    fn query_resource(&self,key: &str) -> ShaderBindingResource {
        self.resource.query_resource(key)
    }

    fn bind_group(&self) -> BindGroupType {
        self.resource.bind_group()
    }
}

#[derive(Debug, Clone)]
pub struct PhongMaterialFaceBuilder {
    normal: InputResource<Vec3f>,
    diffuse: InputResource<Color>,
    specular: InputResource<Color>,
    emissive: InputResource<Color>,
    emissive_strength: f32,
    shininess: f32,
    recv_shadow: bool,

    sampler: Option<ResourceRef>,
    alpha_test: Option<f32>,
}

impl PhongMaterialFaceBuilder {
    pub fn new() -> Self {
        Self {
            normal: InputResource::default(),
            diffuse: InputResource::default(),
            specular: InputResource::default(),
            emissive: InputResource::default(),
            emissive_strength: 1.0f32,
            shininess: 8f32,
            recv_shadow: false,
            alpha_test: None,
            sampler: None,
        }
    }
    pub fn diffuse(mut self, map: InputResource<Color>) -> Self {
        self.set_diffuse(map);
        self
    }
    pub fn set_diffuse(&mut self, map: InputResource<Color>) {
        self.diffuse = map;
    }

    pub fn get_diffuse(&self) -> InputResource<Color> {
        self.diffuse.clone()
    }

    pub fn normal(mut self, map: InputResource<Vec3f>) -> Self {
        self.set_normal(map);
        self
    }
    pub fn set_normal(&mut self, map: InputResource<Vec3f>) {
        self.normal = map;
    }

    pub fn get_normal(&self) -> InputResource<Vec3f> {
        self.normal.clone()
    }

    pub fn specular(mut self, map: InputResource<Color>) -> Self {
        self.set_specular(map);
        self
    }
    pub fn set_specular(&mut self, map: InputResource<Color>) {
        self.specular = map;
    }

    pub fn emissive(mut self, map: InputResource<Color>) -> Self {
        self.set_emissive(map);
        self
    }
    pub fn set_emissive(&mut self, map: InputResource<Color>) {
        self.emissive = map;
    }

    pub fn emissive_strength(mut self, strength: f32) -> Self {
        self.set_emissive_strength(strength);
        self
    }
    pub fn set_emissive_strength(&mut self, strength: f32) {
        self.emissive_strength = strength;
    }

    pub fn shininess(mut self, color: f32) -> Self {
        self.set_shininess(color);
        self
    }
    pub fn set_shininess(&mut self, color: f32) {
        self.shininess = color;
    }

    pub fn alpha_test(mut self, cutoff: f32) -> Self {
        self.set_alpha_test(cutoff);
        self
    }
    pub fn set_alpha_test(&mut self, cutoff: f32) {
        self.alpha_test = Some(cutoff);
    }

    pub fn sampler(mut self, sampler: ResourceRef) -> Self {
        self.set_sampler(sampler);
        self
    }
    pub fn set_sampler(&mut self, sampler: ResourceRef) {
        self.sampler = Some(sampler);
    }
    pub fn has_sampler(&self) -> bool {
        self.sampler.is_some()
    }

    pub fn recv_shadow(mut self) -> Self {
        self.set_recv_shadow();
        self
    }
    pub fn set_recv_shadow(&mut self) {
        self.recv_shadow = true;
    }

    pub fn build(self) -> PhongMaterialFace {
        let mut variants_base = VariantFlagsBuilder::default();
        let mut variants_add = VariantFlagsBuilder::default();
        let resource = BindingResourceMap::new(BindGroupType::Material);

        for ty in self.diffuse.iter() {
            match ty {
                InputResourceIterItem::Constant(c) => {
                    variants_base.add_flag("DIFFUSE_CONSTANT");
                    variants_add.add_flag("DIFFUSE_CONSTANT");
                    resource.upsert("diffuse_color", c);
                }
                InputResourceIterItem::PreVertex => {
                    variants_base.add_flag("DIFFUSE_VERTEX");
                    variants_add.add_flag("DIFFUSE_VERTEX");
                }
                InputResourceIterItem::Texture(t) => {
                    variants_base.add_flag("DIFFUSE_TEXTURE");
                    variants_add.add_flag("DIFFUSE_TEXTURE");
                    resource.upsert("diffuse_texture", t);
                }
                InputResourceIterItem::Instance => {
                    panic!("diffuse instance is not supported");
                }
            }
        }

        for ty in self.specular.iter() {
            match ty {
                InputResourceIterItem::Constant(c) => {
                    variants_base.add_flag("SPECULAR_CONSTANT");
                    variants_add.add_flag("SPECULAR_CONSTANT");
                    resource.upsert("specular_color", c);
                }
                InputResourceIterItem::PreVertex => {
                    variants_base.add_flag("SPECULAR_VERTEX");
                    variants_add.add_flag("SPECULAR_VERTEX");
                }
                InputResourceIterItem::Texture(t) => {
                    variants_base.add_flag("SPECULAR_TEXTURE");
                    variants_add.add_flag("SPECULAR_TEXTURE");
                    resource.upsert("specular_texture", t);
                }
                InputResourceIterItem::Instance => {
                    panic!("specular instance is not supported");
                }
            }
        }

        for ty in self.normal.iter() {
            match ty {
                InputResourceIterItem::Constant(_) => {
                    panic!("normal constant is not supported");
                }
                InputResourceIterItem::PreVertex => {
                    variants_base.add_flag("NORMAL_VERTEX");
                    variants_add.add_flag("NORMAL_VERTEX");
                }
                InputResourceIterItem::Texture(t) => {
                    variants_base.add_flag("NORMAL_TEXTURE");
                    variants_add.add_flag("NORMAL_TEXTURE");
                    resource.upsert("normal_texture", t);
                }
                InputResourceIterItem::Instance => {
                    panic!("normal instance is not supported");
                }
            }
        }

        for ty in self.emissive.iter() {
            match ty {
                InputResourceIterItem::Constant(c) => {
                    variants_base.add_flag("EMISSIVE_CONSTANT");
                    resource.upsert("emissive_color", c);
                    resource.upsert("emissive_strength", self.emissive_strength);
                }
                InputResourceIterItem::PreVertex => {
                    variants_base.add_flag("EMISSIVE_VERTEX");
                }
                InputResourceIterItem::Texture(t) => {
                    variants_base.add_flag("EMISSIVE_TEXTURE");
                    resource.upsert("emissive_texture", t);
                }
                InputResourceIterItem::Instance => {
                    panic!("emissive instance is not supported");
                }
            }
        }

        resource.upsert("shininess", self.shininess);
        resource.upsert("sampler", self.sampler.unwrap());

        PhongMaterialFace {
            resource,

            variants_base: variants_base.build(),
            variants_add: variants_add.build(),
        }
    }
}

impl Default for PhongMaterialFaceBuilder {
    fn default() -> Self {
        Self::new()
    }
}
