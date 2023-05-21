use crate::types::{Color, Vec3u};

#[derive(Debug)]
pub enum ClearValue {
    Color(Color),
    Depth((f32, u32)),
}

impl ClearValue {
    pub fn depth(&self) -> f32 {
        match self {
            ClearValue::Color(_) => todo!(),
            ClearValue::Depth((d, s)) => *d,
        }
    }
    pub fn stencil(&self) -> u32 {
        match self {
            ClearValue::Color(_) => todo!(),
            ClearValue::Depth((d, s)) => *s,
        }
    }
    pub fn color(&self) -> Color {
        match self {
            ClearValue::Color(c) => *c,
            ClearValue::Depth(_) => todo!(),
        }
    }
}

pub type ResourceId = u32;
pub const RT_COLOR_RESOURCE_ID: ResourceId = 0;
pub const RT_DEPTH_RESOURCE_ID: ResourceId = 1;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ResourceOp {
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
    pub id: ResourceId,
    pub clear: Option<ClearValue>,
    pub usage: wgpu::TextureUsages,
}

#[derive(Debug)]
pub struct ImportTextureInfo {
    pub id: ResourceId,
    pub clear: Option<ClearValue>,
}

#[derive(Debug)]
pub struct BufferInfo {
    pub size: u64,
    pub id: ResourceId,
    pub usage: wgpu::BufferUsages,
}

#[derive(Debug)]
pub enum ResourceType {
    Texture(TextureInfo),
    Buffer(BufferInfo),
    ImportTexture((ImportTextureInfo, String)),
    ImportBuffer((ResourceId, String)),
    AliasResource(ResourceId, ResourceId),
}

pub struct Resource {
    pub(crate) refs: u32,
    pub(crate) ty: ResourceType,
}
