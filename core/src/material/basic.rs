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
    texture: Option<ResourceRef>,
}

impl BasicMaterialFace {
    pub fn texture(&self) -> Option<&ResourceRef> {
        self.texture.as_ref().map(|v| v)
    }
    pub fn color(&self) -> Vec4f {
        self.color
    }
    pub fn variants(&self) -> &[tshader::Variant] {
        &self.variants
    }
}

impl MaterialFace for BasicMaterialFace {
    fn shader_id(&self) -> u64 {
        // self.shader as u64
        0
    }
}

#[derive(Default, Clone, Debug)]
pub struct BasicMaterialFaceBuilder {
    primitive: wgpu::PrimitiveState,
    blend: Option<wgpu::BlendState>,
    has_color: bool,
    has_texture: bool,
    texture: Option<ResourceRef>,
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
            primitive: wgpu::PrimitiveState::default(),
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

    pub fn build(mut self) -> BasicMaterialFace {
        let mut variants = vec![];

        let shader = if self.has_color {
            if self.has_texture {
                variants.push(tshader::Variant::TextureColor);
            } else {
                self.texture = None;
            }
        } else {
            if self.has_texture {
            } else {
                self.texture = None;
            }
        };
        BasicMaterialFace {
            color: self.color,
            variants,
            texture: self.texture,
        }
    }
}
