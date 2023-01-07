use super::MaterialFace;

#[derive(Debug)]
pub struct EguiMaterialFace {
    texture: Option<wgpu::TextureView>,
}

impl EguiMaterialFace {
    pub fn texture(&self) -> &wgpu::TextureView {
        self.texture.as_ref().unwrap()
    }
}

impl MaterialFace for EguiMaterialFace {
    fn shader_id(&self) -> u64 {
        0
    }
}

#[derive(Debug, Default)]
pub struct EguiMaterialFaceBuilder {
    texture: Option<wgpu::TextureView>,
}

impl EguiMaterialFaceBuilder {
    pub fn with_texture(mut self, texture: wgpu::TextureView) -> Self {
        self.texture = Some(texture);
        self
    }
    pub fn build(mut self) -> EguiMaterialFace {
        EguiMaterialFace {
            texture: self.texture,
        }
    }
}
