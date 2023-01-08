use std::sync::Arc;

use crate::{
    ds::Texture,
    ps::{BlendState, DepthDescriptor, PrimitiveStateDescriptor},
    types::Vec4f,
};

use super::{Material, MaterialFace, MaterialShader};

#[derive(Debug, Clone, Copy)]
pub enum BasicMaterialShader {
    None,
    Color,
    Texture,
    ColorTexture,
}

impl MaterialShader for BasicMaterialShader {}

#[derive(Debug)]
pub struct BasicMaterialFace {
    color: Vec4f,
    shader: BasicMaterialShader,
    texture: Option<Texture>,
}

impl BasicMaterialFace {
    pub fn shader_ex(&self) -> BasicMaterialShader {
        self.shader
    }
    pub fn texture(&self) -> Option<&Texture> {
        self.texture.as_ref().map(|v| v)
    }
    pub fn color(&self) -> Vec4f {
        self.color
    }
}

impl MaterialFace for BasicMaterialFace {
    fn shader_id(&self) -> u64 {
        self.shader as u64
    }
}

#[derive(Default, Clone, Debug)]
pub struct BasicMaterialFaceBuilder {
    primitive: PrimitiveStateDescriptor,
    blend: Option<BlendState>,
    has_color: bool,
    has_texture: bool,
    texture: Option<Texture>,
    color: Vec4f,
}

impl BasicMaterialFaceBuilder {
    pub fn new() -> Self {
        Self {
            has_color: false,
            has_texture: false,
            texture: None,
            color: Vec4f::new(1f32, 1f32, 1f32, 1f32),
            blend: None,
            primitive: PrimitiveStateDescriptor::default(),
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
    pub fn with_texture_data(mut self, texture: Texture) -> Self {
        self.texture = Some(texture);
        self
    }

    pub fn build(mut self) -> BasicMaterialFace {
        let shader = if self.has_color {
            if self.has_texture {
                BasicMaterialShader::ColorTexture
            } else {
                self.texture = None;
                BasicMaterialShader::Color
            }
        } else {
            if self.has_texture {
                BasicMaterialShader::Texture
            } else {
                self.texture = None;
                BasicMaterialShader::None
            }
        };
        BasicMaterialFace {
            color: self.color,
            shader,
            texture: self.texture,
        }
    }
}
