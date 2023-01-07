use crate::{
    backends::wgpu_backend::{PassEncoder, WGPUResource},
    material::Material,
    scene::{Camera, Scene},
};

use super::common::UniformBinder;

#[derive(Debug)]
pub struct MaterialRenderContext<'a, 'b> {
    pub gpu: &'a WGPUResource,
    pub camera: &'a Camera,
    pub scene: &'a Scene,
    pub encoder: &'a mut PassEncoder<'b>,
    pub hint_fmt: wgpu::TextureFormat,
}

pub trait MaterialRenderer {
    fn new_frame(&mut self, gpu: &WGPUResource);

    fn prepare_render(&mut self, gpu: &WGPUResource, camera: &Camera);

    fn render_material<'a, 'b>(
        &mut self,
        ctx: &mut MaterialRenderContext<'a, 'b>,
        objects: &[u64],
        material: &Material,
    );

    fn sort_key(&mut self, material: &Material, gpu: &WGPUResource) -> u64;
}

pub trait MaterialRendererFactory {
    fn new(&self) -> Box<dyn MaterialRenderer>;
}

struct BufferCache {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
}

pub mod basic;
pub mod egui;

struct MaterialInstance {
    ubo: UniformBinder,
}
