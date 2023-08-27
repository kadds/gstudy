use std::fmt::Debug;

use crate::{
    backends::wgpu_backend::ClearValue,
    types::{Color, Vec3u},
};

pub type ResourceId = u32;
pub const RT_COLOR_RESOURCE_ID: ResourceId = 0;
pub const RT_RESOLVE_COLOR_RESOURCE_ID: ResourceId = 1;
pub const RT_DEPTH_RESOURCE_ID: ResourceId = 2;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResourceUsage {
    TextureRead,
    TextureWrite,
    TextureReadAndWrite,
    PipelineBuffer,
    BufferRead,
    RenderTargetTextureRead,
    RenderTargetTextureWrite,
}

#[derive(Debug)]
pub struct TextureInfo {
    pub size: Vec3u,
    pub format: wgpu::TextureFormat,
    pub clear: Option<ClearValue>,
    pub usage: wgpu::TextureUsages,
    pub sampler_count: u32,
}

#[derive(Debug)]
pub struct ImportTextureInfo {
    pub clear: Option<ClearValue>,
}

#[derive(Debug)]
pub struct BufferInfo {
    pub size: u64,
    pub usage: wgpu::BufferUsages,
}

pub enum ResourceType {
    Texture(TextureInfo),
    Buffer(BufferInfo),
    ImportTexture(ImportTextureInfo),
    ImportBuffer(ResourceId),
    AliasResource(ResourceId, ResourceId),
}

impl Debug for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Texture(_) => f.debug_tuple("Texture").finish(),
            Self::Buffer(_) => f.debug_tuple("Buffer").finish(),
            Self::ImportTexture(_) => f.debug_tuple("ImportTexture").finish(),
            Self::ImportBuffer(_) => f.debug_tuple("ImportBuffer").finish(),
            Self::AliasResource(_, _) => f.debug_tuple("AliasResource").finish(),
        }
    }
}

pub struct ResourceNode {
    pub inner: ResourceType,
    pub name: String,
    pub id: ResourceId,
}

impl Debug for ResourceNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceNode")
            .field("inner", &self.inner)
            .field("name", &self.name)
            .field("id", &self.id)
            .finish()
    }
}
