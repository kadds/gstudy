use std::{any::TypeId, fmt::Debug};

use wgpu::{RenderPipeline, ShaderModuleDescriptor};

use crate::types::*;

pub trait Material: Sync + Send + Debug + 'static {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

#[derive(Debug)]
pub struct BasicMaterial {
    color: Vec4f,
}

impl BasicMaterial {
    pub fn new(color: Vec4f) -> Self {
        Self { color }
    }
    pub fn self_type_id() -> TypeId {
        TypeId::of::<Self>()
    }
}

impl Material for BasicMaterial {}
