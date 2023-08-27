use core::{
    context::ResourceRef,
    material::{MaterialFace, MaterialMap},
    types::{Color, Vec3f, Vec4f},
};
use std::hash::Hasher;

#[derive(Debug)]
pub struct PhongMaterialFace {
    diffuse: MaterialMap<Color>,
    specular: MaterialMap<Color>,
    normal: MaterialMap<Vec3f>,
    sampler: Option<ResourceRef>,

    shininess: f32,

    variants: Vec<tshader::Variant>,
    variants_name: String,
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
        todo!()
    }

    fn has_alpha_test(&self) -> bool {
        todo!()
    }
}

#[derive(Debug, Default)]
pub struct PhongMaterialFaceBuilder {
    normal: MaterialMap<Vec3f>,
    diffuse: MaterialMap<Color>,
    specular: MaterialMap<Color>,
    shininess: f32,

    sampler: Option<ResourceRef>,
}

impl PhongMaterialFaceBuilder {
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
        match self.diffuse {
            MaterialMap::None => {}
            MaterialMap::Constant(_) => {}
            MaterialMap::PreVertex => {
                variants.push(tshader::Variant::VertexColor);
            }
            MaterialMap::Texture(_) => {
                variants.push(tshader::Variant::TextureColor);
            }
        }

        PhongMaterialFace {
            diffuse: self.diffuse,
            specular: self.specular,
            normal: self.normal,
            shininess: self.shininess,

            variants_name: tshader::variants_name(&variants[..]),
            variants,
            sampler: self.sampler,
        }
    }
}
