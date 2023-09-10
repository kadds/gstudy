use core::{
    backends::wgpu_backend::uniform_alignment,
    context::ResourceRef,
    material::{InputResource, InputResourceIterItem, MaterialFace},
    types::{Color, Vec3f, Vec4f},
    util::any_as_u8_slice,
};
use std::{hash::Hasher, io::Write};

// #[repr(C)]
// struct PhongMaterialData {
//     diffuse,
// }

#[derive(Debug)]
pub struct PhongMaterialFace {
    pub diffuse: InputResource<Color>,
    pub specular: InputResource<Color>,
    pub normal: InputResource<Vec3f>,
    pub emissive: InputResource<Color>,
    pub sampler: Option<ResourceRef>,

    pub shininess: f32,

    pub variants: Vec<&'static str>,
    pub variants_add: Vec<&'static str>,
    pub variants_name: String,
    pub uniform: Vec<u8>,
}

impl MaterialFace for PhongMaterialFace {
    fn sort_key(&self) -> u64 {
        let mut hasher = fxhash::FxHasher64::default();
        hasher.write(self.variants_name.as_bytes());

        let sid = hasher.finish();

        let tid = self.diffuse.sort_key();
        let tid2 = self.specular.sort_key();
        let tid3 = self.normal.sort_key();
        let mut hasher2 = fxhash::FxHasher64::default();
        hasher2.write_u64(tid);
        hasher2.write_u64(tid2);
        hasher2.write_u64(tid3);

        (sid & 0xFFFF_FFFF) | (hasher2.finish() >> 32)
    }

    fn hash_key(&self) -> u64 {
        let mut h = fxhash::FxHasher::default();
        h.write(self.variants_name.as_bytes());
        h.finish()
    }

    fn material_uniform(&self) -> &[u8] {
        &self.uniform
    }

    fn has_alpha_test(&self) -> bool {
        false
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

    pub fn build(self) -> PhongMaterialFace {
        let mut variants = vec![];
        let mut variants_add = vec![];
        let mut uniform = vec![];

        for ty in self.diffuse.iter() {
            match ty {
                InputResourceIterItem::Constant(c) => {
                    variants.push("DIFFUSE_CONSTANT");
                    variants_add.push("DIFFUSE_CONSTANT");
                    uniform.write_all(any_as_u8_slice(&Vec4f::new(c.x, c.y, c.z, 0f32)));
                }
                InputResourceIterItem::PreVertex => {
                    variants.push("DIFFUSE_VERTEX");
                    variants_add.push("DIFFUSE_VERTEX");
                }
                InputResourceIterItem::Texture(_) => {
                    variants.push("DIFFUSE_TEXTURE");
                    variants_add.push("DIFFUSE_TEXTURE");
                }
                InputResourceIterItem::Instance => {
                    panic!("diffuse instance is not supported");
                }
            }
        }

        for ty in self.specular.iter() {
            match ty {
                InputResourceIterItem::Constant(c) => {
                    variants.push("SPECULAR_CONSTANT");
                    variants_add.push("SPECULAR_CONSTANT");
                    uniform.write_all(any_as_u8_slice(&Vec4f::new(c.x, c.y, c.z, 0f32)));
                }
                InputResourceIterItem::PreVertex => {
                    variants.push("SPECULAR_VERTEX");
                    variants_add.push("SPECULAR_VERTEX");
                }
                InputResourceIterItem::Texture(_) => {
                    variants.push("SPECULAR_TEXTURE");
                    variants_add.push("SPECULAR_TEXTURE");
                }
                InputResourceIterItem::Instance => {
                    panic!("specular instance is not supported");
                }
            }
        }

        for ty in self.normal.iter() {
            match ty {
                InputResourceIterItem::Constant(c) => {
                    panic!("normal constant is not supported");
                }
                InputResourceIterItem::PreVertex => {
                    variants.push("NORMAL_VERTEX");
                    variants_add.push("NORMAL_VERTEX");
                }
                InputResourceIterItem::Texture(_) => {
                    variants.push("NORMAL_TEXTURE");
                    variants_add.push("NORMAL_TEXTURE");
                }
                InputResourceIterItem::Instance => {
                    panic!("normal instance is not supported");
                }
            }
        }

        uniform.write_all(any_as_u8_slice(&Vec4f::default()));
        for ty in self.emissive.iter() {
            match ty {
                InputResourceIterItem::Constant(c) => {
                    variants.push("EMISSIVE_CONSTANT");
                    uniform.truncate(uniform.len() - std::mem::size_of::<Vec4f>());
                    uniform.write_all(any_as_u8_slice(&Vec4f::new(
                        c.x,
                        c.y,
                        c.z,
                        self.emissive_strength,
                    )));
                }
                InputResourceIterItem::PreVertex => {
                    variants.push("EMISSIVE_VERTEX");
                }
                InputResourceIterItem::Texture(_) => {
                    variants.push("EMISSIVE_TEXTURE");
                }
                InputResourceIterItem::Instance => {
                    panic!("emissive instance is not supported");
                }
            }
        }

        uniform.write_all(any_as_u8_slice(&self.shininess));
        uniform_alignment(&mut uniform);

        PhongMaterialFace {
            diffuse: self.diffuse,
            specular: self.specular,
            normal: self.normal,
            shininess: self.shininess,
            emissive: self.emissive,
            uniform,

            variants_name: tshader::variants_name(&variants[..]),
            variants,
            variants_add,
            sampler: self.sampler,
        }
    }
}

impl Default for PhongMaterialFaceBuilder {
    fn default() -> Self {
        Self::new()
    }
}
