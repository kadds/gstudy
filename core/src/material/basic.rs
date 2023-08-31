use std::{fmt::Debug, hash::Hasher};

use crate::{
    context::ResourceRef,
    types::{Color, Vec3f, Vec4f},
    util::any_as_u8_slice,
};

use super::{MaterialFace, MaterialMap};

#[repr(C)]
#[derive(Default)]
pub struct Parameter {}

#[repr(C)]
#[derive(Default)]
pub struct ParameterWithAlpha {
    pub alpha: f32,
    pub _pad: Vec3f,
}

#[repr(C)]
#[derive(Default)]
pub struct ConstParameter {
    pub color: Vec4f,
}

#[repr(C)]
#[derive(Default)]
pub struct ConstParameterWithAlpha {
    pub color: Vec4f,
    pub alpha: f32,
    pub _pad: Vec3f,
}

enum BasicMaterialParameter {
    Default(Parameter),
    ConstParameter(ConstParameter),
    ConstParameterWithAlpha(ConstParameterWithAlpha),
    Alpha(ParameterWithAlpha),
}

pub struct BasicMaterialFace {
    pub(crate) variants: Vec<&'static str>,
    pub(crate) variants_name: String,
    pub(crate) texture: MaterialMap<Color>,
    pub(crate) sampler: Option<ResourceRef>,
    pub(crate) alpha_test: Option<f32>,

    parameter: BasicMaterialParameter,
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

    fn material_data(&self) -> &[u8] {
        match &self.parameter {
            BasicMaterialParameter::Default(_) => &[],
            BasicMaterialParameter::ConstParameter(p) => any_as_u8_slice(p),
            BasicMaterialParameter::ConstParameterWithAlpha(p) => any_as_u8_slice(p),
            BasicMaterialParameter::Alpha(p) => any_as_u8_slice(p),
        }
    }
}

#[derive(Default, Clone, Debug)]
pub struct BasicMaterialFaceBuilder {
    alpha_test: Option<f32>,
    sampler: Option<ResourceRef>,
    texture: MaterialMap<Color>,
}

impl BasicMaterialFaceBuilder {
    pub fn new() -> Self {
        Self {
            alpha_test: None,
            ..Default::default()
        }
    }
    pub fn texture(mut self, texture: MaterialMap<Color>) -> Self {
        self.texture = texture;
        self
    }
    pub fn sampler(mut self, sampler: ResourceRef) -> Self {
        self.sampler = Some(sampler);
        self
    }
    pub fn alpha_test(&mut self, cut: f32) {
        self.alpha_test = Some(cut);
    }

    pub fn build(self) -> BasicMaterialFace {
        let mut variants = vec![];
        let parameter = match self.texture {
            MaterialMap::None => {
                if let Some(a) = self.alpha_test {
                    BasicMaterialParameter::Alpha(ParameterWithAlpha {
                        alpha: a,
                        ..Default::default()
                    })
                } else {
                    BasicMaterialParameter::Default(Parameter {})
                }
            }
            MaterialMap::Constant(c) => {
                variants.push("CONST_COLOR");
                if let Some(a) = self.alpha_test {
                    BasicMaterialParameter::ConstParameterWithAlpha(ConstParameterWithAlpha {
                        alpha: a,
                        color: c,
                        ..Default::default()
                    })
                } else {
                    BasicMaterialParameter::ConstParameter(ConstParameter { color: c })
                }
            }
            MaterialMap::PreVertex => {
                variants.push("VERTEX_COLOR");

                if let Some(a) = self.alpha_test {
                    BasicMaterialParameter::Alpha(ParameterWithAlpha {
                        alpha: a,
                        ..Default::default()
                    })
                } else {
                    BasicMaterialParameter::Default(Parameter {})
                }
            }
            MaterialMap::Texture(_) => {
                variants.push("TEXTURE");

                if let Some(a) = self.alpha_test {
                    BasicMaterialParameter::Alpha(ParameterWithAlpha {
                        alpha: a,
                        ..Default::default()
                    })
                } else {
                    BasicMaterialParameter::Default(Parameter {})
                }
            }
        };

        if self.alpha_test.is_some() {
            variants.push("ALPHA_TEST");
        }

        BasicMaterialFace {
            variants_name: tshader::variants_name(&variants[..]),
            variants,
            texture: self.texture,
            sampler: self.sampler,
            alpha_test: self.alpha_test,
            parameter,
        }
    }
}
