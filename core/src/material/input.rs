
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{context::ResourceRef};


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


#[derive(Debug, Default, Clone, PartialEq, PartialOrd)]
pub struct InputAlphaTest {
    cutoff: Option<f32>,
}

impl InputAlphaTest {
    pub fn make_disabled() -> Self {
        Self {
            cutoff: None,
        }
    }

    pub fn make_enabled(cutoff: f32) -> Self {
        Self {
            cutoff: Some(cutoff),
        }
    }

    pub fn cutoff(&self) -> Option<f32> {
        self.cutoff.clone()
    }

    pub fn is_enable(&self) -> bool {
        self.cutoff.is_some()
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
