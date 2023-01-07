use crate::backends::wgpu_backend::WGPUResource;

#[derive(Debug, Clone)]
pub struct Texture {}

impl Texture {
    pub(crate) fn internal_view(&self) -> &wgpu::TextureView {
        todo!()
    }
}
