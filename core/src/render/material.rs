use std::{
    any::{Provider, TypeId},
    collections::HashMap,
    io::{BufReader, Read},
    sync::{Arc, Mutex},
};

use tshader::ShaderTech;

use crate::{
    backends::wgpu_backend::WGPUResource,
    graph::rdg::{RenderGraph, RenderGraphBuilder},
    material::Material,
    scene::{Camera, Scene},
};

use super::PassIdent;

pub struct MaterialRenderContext<'a> {
    pub gpu: &'a WGPUResource,
    pub scene: &'a Scene,
    pub camera_uniform: &'a wgpu::Buffer,
}

pub trait MaterialRenderer {
    fn render_material<'a, 'b>(
        &'a mut self,
        ctx: &'b mut MaterialRenderContext<'b>,
        objects: &'b [u64],
        material: &'b Material,
        encoder: &mut wgpu::CommandEncoder,
    );
}

pub struct SetupResource<'a> {
    pub ui_camera: &'a wgpu::Buffer,
    pub main_camera: &'a wgpu::Buffer,
    pub shader_loader: &'a tshader::Loader,
}

pub trait MaterialRendererFactory {
    fn setup(
        &self,
        pass_ident: PassIdent,
        material: &[&Material],
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) -> Arc<Mutex<dyn MaterialRenderer>>;
    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64;
}

struct BufferCache {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
}

// pub mod basic;
pub mod egui;
// pub mod basic;

pub struct HardwareMaterialShaderResource {
    pub pass: smallvec::SmallVec<[Arc<wgpu::RenderPipeline>; 1]>,
}

pub type MaterialResourceId = u64;
