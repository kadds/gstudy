use std::{hash::Hasher, sync::Arc};

use crate::{
    context::ResourceRef,
    types::{Vec2f, Vec3f, Vec4f},
};

use super::MaterialFace;

#[repr(C)]
pub struct ConstParameter {
    pub color: Vec4f,
}

#[repr(C)]
pub struct ConstParameterWithAlpha {
    pub color: Vec4f,
    pub alpha: f32,
    pub _pad: Vec3f,
}

#[derive(Debug)]
pub struct BasicMaterialFace {
    color: Vec4f,
    variants: Vec<tshader::Variant>,
    variants_name: String,
    texture: Option<ResourceRef>,
}

impl BasicMaterialFace {
    pub fn texture(&self) -> Option<&ResourceRef> {
        self.texture.as_ref()
    }
    pub fn color(&self) -> Vec4f {
        self.color
    }
    pub fn variants(&self) -> &[tshader::Variant] {
        &self.variants
    }
    pub fn variants_name(&self) -> &str {
        &self.variants_name
    }
}

impl MaterialFace for BasicMaterialFace {
    fn sort_key(&self) -> u64 {
        let tid = if let Some(texture) = &self.texture {
            let mut hasher = fxhash::FxHasher64::default();
            hasher.write_u64(texture.id());
            hasher.finish()
        } else {
            0
        };

        let mut hasher = fxhash::FxHasher64::default();
        hasher.write(self.variants_name.as_bytes());

        let sid = hasher.finish();

        (sid & 0xFFFF_FFFF) | (tid >> 32)
    }
}

#[derive(Default, Clone, Debug)]
pub struct BasicMaterialFaceBuilder {
    has_color: bool,
    has_texture: bool,
    has_alpha_test: bool,
    texture: Option<ResourceRef>,
    color: Vec4f,
}

impl BasicMaterialFaceBuilder {
    pub fn new() -> Self {
        Self {
            has_color: false,
            has_texture: false,
            has_alpha_test: false,
            texture: None,
            color: Vec4f::new(1f32, 1f32, 1f32, 1f32),
        }
    }
    pub fn with_color(mut self) -> Self {
        self.has_color = true;
        self
    }
    pub fn with_texture(mut self) -> Self {
        self.has_texture = true;
        self
    }
    pub fn with_constant_color(mut self, color: Vec4f) -> Self {
        self.color = color;
        self
    }
    pub fn with_texture_data(mut self, texture: ResourceRef) -> Self {
        self.texture = Some(texture);
        self
    }
    pub fn enable_alpha_test(&mut self) {
        self.has_alpha_test = true;
    }

    pub fn build(mut self) -> BasicMaterialFace {
        let mut variants = vec![];
        if self.has_texture {
            variants.push(tshader::Variant::TextureColor);
        } else {
            self.texture = None;
        }

        if self.has_color {
            variants.push(tshader::Variant::VertexColor);
        }
        if self.has_alpha_test {
            variants.push(tshader::Variant::AlphaTest);
        }

        BasicMaterialFace {
            color: self.color,
            variants_name: tshader::variants_name(&variants[..]),
            variants,
            texture: self.texture,
        }
    }
}
