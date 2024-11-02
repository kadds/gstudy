use std::{collections::HashMap, sync::Mutex};

use crate::{context::ResourceRef, render::pso::BindGroupType, types::{Vec2f, Vec3f, Vec4f}};


#[derive(Debug, Clone)]
pub enum ShaderBindingResource {
    Nothing,
    Resource(ResourceRef),
    Int32(i32),
    Int64(i64),
    Float(f32),
    Double(f64),
    Float2(Vec2f),
    Float3(Vec3f),
    Float4(Vec4f),
}

impl From<ResourceRef> for ShaderBindingResource {
    fn from(value: ResourceRef) -> Self {
        Self::Resource(value)
    }
}

impl From<&ResourceRef> for ShaderBindingResource {
    fn from(value: &ResourceRef) -> Self {
        Self::Resource(value.clone())
    }
}

impl From<i32> for ShaderBindingResource {
    fn from(value: i32) -> Self {
        Self::Int32(value)
    }
}

impl From<i64> for ShaderBindingResource {
    fn from(value: i64) -> Self {
        Self::Int64(value)
    }
}

impl From<f32> for ShaderBindingResource {
    fn from(value: f32) -> Self {
        Self::Float(value)
    }
}

impl From<f64> for ShaderBindingResource {
    fn from(value: f64) -> Self {
        Self::Double(value)
    }
}

impl From<Vec2f> for ShaderBindingResource {
    fn from(value: Vec2f) -> Self {
        Self::Float2(value)
    }
}

impl From<&Vec2f> for ShaderBindingResource {
    fn from(value: &Vec2f) -> Self {
        Self::Float2(*value)
    }
}

impl From<Vec3f> for ShaderBindingResource {
    fn from(value: Vec3f) -> Self {
        Self::Float3(value)
    }
}

impl From<&Vec3f> for ShaderBindingResource {
    fn from(value: &Vec3f) -> Self {
        Self::Float3(*value)
    }
}

impl From<Vec4f> for ShaderBindingResource {
    fn from(value: Vec4f) -> Self {
        Self::Float4(value)
    }
}

impl From<&Vec4f> for ShaderBindingResource {
    fn from(value: &Vec4f) -> Self {
        Self::Float4(*value)
    }
}

impl ShaderBindingResource {
    // pub fn wrap_resource(r: Option<ResourceRef>) -> Self {
    //     if let Some(v) = r {
    //         return Self::ResourceRef(v);
    //     }
    //     Self::None
    // }
}

#[auto_impl::auto_impl(&, Box, Rc, Arc)]
pub trait BindingResourceProvider {
    fn query_resource(&self, key: &str) -> ShaderBindingResource;
    fn bind_group(&self) -> BindGroupType;
}

#[derive(Debug)]
pub struct BindingResourceMap {
    map: Mutex<HashMap<String, ShaderBindingResource>>,
    ty: BindGroupType,
}

impl BindingResourceMap {
    pub fn new(ty: BindGroupType) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            ty,
        }
    }

    pub fn upsert<R: Into<ShaderBindingResource>>(&self, key: &str, res: R) {
        let res = res.into();
        let mut r = self.map.lock().unwrap();
        r.entry(key.to_string())
            .and_modify(|v| *v = res.clone())
            .or_insert(res);
    }
}

impl BindingResourceProvider for BindingResourceMap {
    fn query_resource(&self, key: &str) -> ShaderBindingResource {
        let r = self.map.lock().unwrap();
        r.get(key).cloned().unwrap_or(ShaderBindingResource::Nothing)
    }
    fn bind_group(&self) -> BindGroupType {
        self.ty
    }
}
