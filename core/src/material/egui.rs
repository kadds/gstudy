use std::hash::Hasher;

use crate::context::ResourceRef;

use super::MaterialFace;

#[derive(Debug)]
pub struct EguiMaterialFace {
    texture: Option<ResourceRef>,
}

impl EguiMaterialFace {
    pub fn texture(&self) -> ResourceRef {
        self.texture.as_ref().unwrap().clone()
    }
}

impl MaterialFace for EguiMaterialFace {
    fn sort_key(&self) -> u64 {
        if let Some(texture) = &self.texture {
            let mut hasher = fxhash::FxHasher64::default();
            hasher.write_u64(texture.id());
            return hasher.finish();
        }
        0
    }
}

#[derive(Debug, Default)]
pub struct EguiMaterialFaceBuilder {
    texture: Option<ResourceRef>,
}

impl EguiMaterialFaceBuilder {
    pub fn with_texture(mut self, texture: ResourceRef) -> Self {
        self.texture = Some(texture);
        self
    }
    pub fn build(mut self) -> EguiMaterialFace {
        EguiMaterialFace {
            texture: self.texture,
        }
    }
}
