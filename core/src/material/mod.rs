use std::{
    any::{Any, TypeId},
    fmt::Debug,
    hash::{Hash, Hasher},
    sync::Arc,
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    context::{RContext, ResourceRef},
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

pub trait MaterialFace: Any + Sync + Send + Debug {
    fn sort_key(&self) -> u64;
    fn hash_key(&self) -> u64;
    fn material_uniform(&self) -> &[u8];
    fn has_alpha_test(&self) -> bool;
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
    cached_hash: u64,
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
        self.face.has_alpha_test()
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

    pub fn hash_key(&self) -> u64 {
        self.cached_hash
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
            // RenderDescriptorObject
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

    pub fn build(mut self, context: &RContext) -> Arc<Material> {
        let face = self.face.take().unwrap();
        let hash = face.hash_key();

        let mut h = fxhash::FxHasher::default();
        h.write_u64(hash);
        self.primitive.hash(&mut h);
        if let Some(blend) = &self.blend {
            blend.hash(&mut h);
        }
        let cached_hash = h.finish();

        Arc::new(Material {
            name: self.name,
            id: MaterialId::new(context.alloc_material_id()),
            primitive: self.primitive,
            blend: self.blend,
            face,
            cached_hash,
        })
    }
}

pub trait MaterialShader: Any + Sync + Send + Debug + 'static {}

pub mod basic;

#[derive(Debug, Clone, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum InputResourceType {
    Constant,
    PreVertex,
    Texture,
    Instance,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum InputResourceIterItem<'a, T> {
    Constant(&'a T),
    PreVertex,
    Texture(&'a ResourceRef),
    Instance,
}

pub struct InputResourceIter<'a, T> {
    inner: &'a InputResource<T>,
    idx: i8,
}

impl<'a, T> Iterator for InputResourceIter<'a, T> {
    type Item = InputResourceIterItem<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= 4 {
            return None;
        }
        let next = if self.idx < 0 {
            self.inner.bitmap.first_index()?
        } else {
            self.inner.bitmap.next_index(self.idx as usize)?
        };
        self.idx = next as i8;
        let ty = InputResourceType::try_from_primitive(next as u8).unwrap();
        Some(match ty {
            InputResourceType::Constant => InputResourceIterItem::Constant(&self.inner.constant),
            InputResourceType::PreVertex => InputResourceIterItem::PreVertex,
            InputResourceType::Texture => {
                InputResourceIterItem::Texture(self.inner.texture.as_ref().unwrap())
            }
            InputResourceType::Instance => InputResourceIterItem::Instance,
        })
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Hash)]
pub struct InputResourceBits {
    bitmap: bitmaps::Bitmap<4>,
}

#[derive(Debug, Clone, Default)]
pub struct InputResource<T> {
    bitmap: bitmaps::Bitmap<4>,
    constant: T,
    texture: Option<ResourceRef>,
}

impl<T> InputResource<T> {
    pub fn bits(&self) -> InputResourceBits {
        InputResourceBits {
            bitmap: self.bitmap,
        }
    }
    pub fn is_texture(&self) -> bool {
        let num: u8 = InputResourceType::Texture.into();
        self.bitmap.get(num as usize)
    }
    pub fn is_constant(&self) -> bool {
        let num: u8 = InputResourceType::Constant.into();
        self.bitmap.get(num as usize)
    }

    pub fn sort_key(&self) -> u64 {
        if self.is_texture() {
            return self.texture.as_ref().map(|v| v.id()).unwrap_or_default();
        }
        0
    }

    pub fn iter(&self) -> InputResourceIter<T> {
        InputResourceIter {
            inner: &self,
            idx: -1,
        }
    }

    pub fn texture_ref(&self) -> Option<&ResourceRef> {
        if self.is_texture() {
            Some(self.texture.as_ref().unwrap())
        } else {
            None
        }
    }

    pub fn merge(&mut self, rhs: &Self)
    where
        T: Clone,
    {
        if self.is_texture() && rhs.is_texture() {
            panic!("merge texture fail")
        }
        if self.is_constant() && rhs.is_constant() {
            panic!("merge constant fail")
        }

        if !self.is_texture() {
            self.texture = rhs.texture.clone();
        }

        if !self.is_constant() {
            self.constant = rhs.constant.clone();
        }

        self.bitmap |= rhs.bitmap;
    }

    pub fn merge_available(&mut self, rhs: &Self)
    where
        T: Clone,
    {
        if !self.is_texture() {
            self.texture = rhs.texture.clone();
        }

        if !self.is_constant() {
            self.constant = rhs.constant.clone();
        }

        self.bitmap |= rhs.bitmap;
    }
}

pub struct InputResourceBuilder<T> {
    m: InputResource<T>,
}

impl<T> InputResourceBuilder<T>
where
    T: Copy + Clone + Default,
{
    pub fn new() -> Self {
        Self {
            m: InputResource {
                bitmap: bitmaps::Bitmap::new(),
                constant: T::default(),
                texture: None,
            },
        }
    }
    pub fn add_pre_vertex(&mut self) {
        let num: u8 = InputResourceType::PreVertex.into();
        self.m.bitmap.set(num as usize, true);
    }
    pub fn add_instance(&mut self) {
        let num: u8 = InputResourceType::Instance.into();
        self.m.bitmap.set(num as usize, true);
    }

    pub fn add_constant(&mut self, constant: T) {
        let num: u8 = InputResourceType::Constant.into();
        self.m.bitmap.set(num as usize, true);
        self.m.constant = constant;
    }

    pub fn add_texture(&mut self, texture: ResourceRef) {
        let num: u8 = InputResourceType::Texture.into();
        self.m.bitmap.set(num as usize, true);
        self.m.texture = Some(texture);
    }

    pub fn with_pre_vertex(mut self) -> Self {
        self.add_pre_vertex();
        self
    }

    pub fn with_constant(mut self, constant: T) -> Self {
        self.add_constant(constant);
        self
    }

    pub fn with_instance(mut self) -> Self {
        self.add_instance();
        self
    }

    pub fn with_texture(mut self, texture: ResourceRef) -> Self {
        self.add_texture(texture);
        self
    }

    pub fn build(self) -> InputResource<T> {
        self.m
    }

    pub fn only_pre_vertex() -> InputResource<T> {
        Self::new().with_pre_vertex().build()
    }

    pub fn only_instance() -> InputResource<T> {
        Self::new().with_instance().build()
    }

    pub fn only_texture(texture: ResourceRef) -> InputResource<T> {
        Self::new().with_texture(texture).build()
    }

    pub fn only_constant(c: T) -> InputResource<T> {
        Self::new().with_constant(c).build()
    }
}

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
