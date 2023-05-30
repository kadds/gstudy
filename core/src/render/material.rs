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
    pub main_camera: &'a wgpu::Buffer,
}

pub trait MaterialRenderer {
    fn before_render(&mut self);

    fn render_material<'b>(
        &mut self,
        ctx: &'b mut MaterialRenderContext<'b>,
        objects: &'b [u64],
        material: &'b Material,
        encoder: &mut wgpu::CommandEncoder,
    );

    fn finish_render(&mut self);
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

pub mod basic;
pub mod egui;

pub struct HardwareMaterialShaderResource {
    pub pass: smallvec::SmallVec<[Arc<wgpu::RenderPipeline>; 1]>,
}
