use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::Hash,
    sync::Arc,
};

use crate::context::RContext;

#[derive(Debug, Hash, Eq, PartialEq, Clone, Copy)]
pub struct MaterialId(u64);

impl MaterialId {
    pub fn new(m: u64) -> Self {
        Self(m)
    }

    pub fn id(&self) -> u64 {
        self.0
    }
}

pub trait MaterialFace: Any + Sync + Send + Debug {
    // instance
    fn shader_id(&self) -> u64;
}

#[derive(Debug)]
pub struct Material {
    id: MaterialId,
    primitive: wgpu::PrimitiveState,
    blend: Option<wgpu::BlendState>,
    alpha_test: Option<f32>,

    face: Box<dyn MaterialFace>, // material face
}

impl Material {
    pub fn primitive(&self) -> &wgpu::PrimitiveState {
        &self.primitive
    }
    pub fn blend(&self) -> Option<&wgpu::BlendState> {
        self.blend.as_ref()
    }
    pub fn alpha_test(&self) -> Option<f32> {
        self.alpha_test
    }

    pub fn is_transparent(&self) -> bool {
        self.blend.is_some()
    }

    pub fn id(&self) -> MaterialId {
        self.id
    }

    pub fn face(&self) -> &dyn MaterialFace {
        self.face.as_ref()
    }

    pub fn face_id(&self) -> TypeId {
        self.face.as_ref().type_id()
    }

    pub fn face_by<M: MaterialFace>(&self) -> &M {
        (self.face.as_ref() as &dyn Any)
            .downcast_ref::<M>()
            .unwrap()
    }
}

#[derive(Debug, Default)]
pub struct MaterialBuilder {
    primitive: wgpu::PrimitiveState,
    blend: Option<wgpu::BlendState>,
    alpha_test: Option<f32>,
    face: Option<Box<dyn MaterialFace>>,
}

impl Clone for MaterialBuilder {
    fn clone(&self) -> Self {
        Self {
            primitive: self.primitive,
            blend: self.blend,
            alpha_test: self.alpha_test,
            face: None,
        }
    }
}

impl MaterialBuilder {
    pub fn with_blend(mut self, blend: wgpu::BlendState) -> Self {
        self.blend = Some(blend);
        self
    }
    pub fn with_primitive(mut self, primitive: wgpu::PrimitiveState) -> Self {
        self.primitive = primitive;
        self
    }

    pub fn with_face<MF: MaterialFace>(mut self, face: MF) -> Self {
        self.face = Some(Box::new(face));
        self
    }

    pub fn with_alpha_test(mut self, cut: f32) -> Self {
        self.alpha_test = Some(cut);
        self
    }

    pub fn build(mut self, context: &RContext) -> Arc<Material> {
        let face = self.face.take().unwrap();
        Arc::new(Material {
            id: MaterialId::new(context.alloc_material_id()),
            primitive: self.primitive,
            alpha_test: self.alpha_test,
            blend: self.blend,
            face,
        })
    }
}

pub trait MaterialShader: Any + Sync + Send + Debug + 'static {}

pub mod basic;
pub mod egui;
