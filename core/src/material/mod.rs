use std::{
    any::{Any, TypeId}, fmt::Debug, hash::Hash, sync::{Arc, Mutex}
};

use bind::{BindingResourceProvider, ShaderBindingResource};
use tshader::VariantFlags;

use crate::{
    context::RContext,
    mesh::builder::{InstancePropertyType, MeshPropertyType, PropertiesFrame},
};

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

pub trait MaterialFace: Any + Sync + Send + Debug + BindingResourceProvider {
    fn name(&self) -> &str;

    fn sort_key(&self) -> u64;

    fn variants(&self) -> &VariantFlags;

    fn validate(
        &self,
        t: &PropertiesFrame<MeshPropertyType>,
        i: Option<&PropertiesFrame<InstancePropertyType>>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct Material {
    id: MaterialId,
    name: String,
    primitive: wgpu::PrimitiveState,
    blend: Option<wgpu::BlendState>,

    face: Box<dyn MaterialFace>, // material face

    mutable_face: Mutex<Option<Box<dyn MaterialFace>>>,
}

pub type MaterialArc = Arc<Material>;

impl BindingResourceProvider for Material {
    fn query_resource(&self, name: &str) -> ShaderBindingResource {
        self.face.query_resource(name)
    }
    fn bind_group(&self) -> crate::render::pso::BindGroupType {
        crate::render::pso::BindGroupType::Material
    }
}

impl Material {
    pub fn primitive(&self) -> &wgpu::PrimitiveState {
        &self.primitive
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn blend(&self) -> Option<&wgpu::BlendState> {
        self.blend.as_ref()
    }

    pub fn is_transparent(&self) -> bool {
        self.blend.is_some()
    }

    pub fn has_alpha_test(&self) -> bool {
        if let ShaderBindingResource::Float(_) = self.face.query_resource("alpha_test") {
            return true
        }
        false
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

#[derive(Debug)]
pub struct MaterialBuilder {
    name: String,
    primitive: wgpu::PrimitiveState,
    blend: Option<wgpu::BlendState>,
    face: Option<Box<dyn MaterialFace>>,
}

impl Default for MaterialBuilder {
    fn default() -> Self {
        Self {
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            blend: Default::default(),
            face: Default::default(),
            name: "".to_owned(),
        }
    }
}

impl Clone for MaterialBuilder {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            primitive: self.primitive,
            blend: self.blend,
            face: None,
        }
    }
}

impl MaterialBuilder {
    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = name.into();
        self
    }
    pub fn set_name<S: Into<String>>(&mut self, name: S) {
        self.name = name.into();
    }
    pub fn blend(mut self, blend: wgpu::BlendState) -> Self {
        self.blend = Some(blend);
        self
    }
    pub fn set_blend(&mut self, blend: wgpu::BlendState) {
        self.blend = Some(blend);
    }
    pub fn primitive(mut self, primitive: wgpu::PrimitiveState) -> Self {
        self.primitive = primitive;
        self
    }
    pub fn set_primitive(&mut self, primitive: wgpu::PrimitiveState) {
        self.primitive = primitive;
    }

    pub fn face<MF: MaterialFace>(mut self, face: MF) -> Self {
        self.face = Some(Box::new(face));
        self
    }
    pub fn set_face<MF: MaterialFace>(&mut self, face: MF) {
        self.face = Some(Box::new(face));
    }

    pub fn build(mut self, context: &RContext) -> MaterialArc {
        let face = self.face.take().unwrap();

        Arc::new(Material {
            name: self.name,
            id: MaterialId::new(context.alloc_material_id()),
            primitive: self.primitive,
            blend: self.blend,
            face: face,
            mutable_face: Mutex::new(None),
        })
    }
}

pub trait MaterialShader: Any + Sync + Send + Debug + 'static {}

pub mod basic;
pub mod input;
pub mod bind;

pub fn validate_material_properties(
    t: &PropertiesFrame<MeshPropertyType>,
    i: Option<&PropertiesFrame<InstancePropertyType>>,
    et: &[MeshPropertyType],
    ei: &[InstancePropertyType],
) -> anyhow::Result<()> {
    for (index, prop) in t.properties.iter().enumerate() {
        if index < et.len() {
            if *prop != et[index] {
                anyhow::bail!(
                    "validate material properties fail at index {}, expect {:?}, get {:?}",
                    index,
                    et[index],
                    *prop
                );
            }
        } else {
            anyhow::bail!(
                "validate material properties fail at index {}, expect null, get {:?}",
                index,
                *prop
            );
        }
    }

    if let Some(i) = &i {
        for (index, prop) in i.properties.iter().enumerate() {
            if index < ei.len() {
                if *prop != ei[index] {
                    anyhow::bail!(
                        "validate instance properties fail at index {}, expect {:?}, get {:?}",
                        index,
                        et[index],
                        *prop
                    );
                }
            } else {
                anyhow::bail!(
                    "validate instance properties fail at index {}, expect null, get {:?}",
                    index,
                    *prop
                );
            }
        }
    } else {
        if ei.len() != 0 {
            anyhow::bail!("validate instance properties fail, no instance found");
        }
    }

    Ok(())
}
