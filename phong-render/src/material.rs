use core::{
    context::ResourceRef,
    material::{MaterialFace, MaterialMap},
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
    diffuse: MaterialMap<Color>,
    specular: MaterialMap<Color>,
    normal: MaterialMap<Vec3f>,
    emissive: MaterialMap<Color>,
    sampler: Option<ResourceRef>,

    shininess: f32,

    variants: Vec<&'static str>,
    variants_name: String,
    uniform: Vec<u8>,
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

    fn material_data(&self) -> &[u8] {
        &self.uniform
    }

    fn has_alpha_test(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub struct PhongMaterialFaceBuilder {
    normal: MaterialMap<Vec3f>,
    diffuse: MaterialMap<Color>,
    specular: MaterialMap<Color>,
    emissive: MaterialMap<Color>,
    shininess: f32,

    sampler: Option<ResourceRef>,
}

impl PhongMaterialFaceBuilder {
    pub fn new() -> Self {
        Self {
            normal: MaterialMap::None,
            diffuse: MaterialMap::None,
            specular: MaterialMap::None,
            emissive: MaterialMap::None,
            shininess: 2f32,
            sampler: None,
        }
    }
    pub fn diffuse(mut self, map: MaterialMap<Color>) -> Self {
        self.diffuse = map;
        self
    }
    pub fn normal(mut self, map: MaterialMap<Vec3f>) -> Self {
        self.normal = map;
        self
    }

    pub fn specular(mut self, map: MaterialMap<Color>) -> Self {
        self.specular = map;
        self
    }

    pub fn emissive(mut self, map: MaterialMap<Color>) -> Self {
        self.emissive = map;
        self
    }

    pub fn shininess(mut self, color: f32) -> Self {
        self.shininess = color;
        self
    }

    pub fn sampler(mut self, sampler: ResourceRef) -> Self {
        self.sampler = Some(sampler);
        self
    }

    pub fn build(self) -> PhongMaterialFace {
        let mut variants = vec![];
        let mut uniform = vec![];

        match self.diffuse {
            MaterialMap::None => {}
            MaterialMap::Constant(c) => {
                variants.push("DIFFUSE_CONSTANT");
                uniform.write_all(any_as_u8_slice(&Vec4f::new(c.x, c.y, c.z, 0f32)));
            }
            MaterialMap::PreVertex => {
                variants.push("DIFFUSE_VERTEX");
            }
            MaterialMap::Texture(_) => {
                variants.push("DIFFUSE_TEXTURE");
            }
        }

        uniform.write_all(any_as_u8_slice(&self.shininess));

        PhongMaterialFace {
            diffuse: self.diffuse,
            specular: self.specular,
            normal: self.normal,
            shininess: self.shininess,
            emissive: self.emissive,
            uniform,

            variants_name: tshader::variants_name(&variants[..]),
            variants,
            sampler: self.sampler,
        }
    }
}

impl Default for PhongMaterialFaceBuilder {
    fn default() -> Self {
        Self::new()
    }
}
