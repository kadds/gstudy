use std::{
    any::{Provider, TypeId},
    collections::HashMap,
    io::{BufReader, Read},
    sync::Arc,
};

use crate::{
    backends::wgpu_backend::{PipelinePass, PipelineReflector, WGPUResource},
    graph::rdg::{RenderGraph, RenderGraphBuilder},
    material::Material,
    scene::{Camera, Scene},
};

use super::PassIdent;

pub struct MaterialRenderContext<'a> {
    pub gpu: &'a WGPUResource,
    pub scene: &'a Scene,
    pub cache: &'a HardwareMaterialShaderCache,
}

pub trait MaterialRenderer {
    fn render_material<'a, 'b>(
        &'a self,
        ctx: &'b mut MaterialRenderContext<'b>,
        objects: &'b [u64],
        material: &'b Material,
        encoder: &mut wgpu::CommandEncoder,
    );
}

pub trait MaterialRendererFactory {
    fn setup(
        &self,
        pass_ident: PassIdent,
        material: &[&Material],
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        cache: &mut HardwareMaterialShaderCache,
    ) -> Arc<dyn MaterialRenderer>;
    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64;
}

struct BufferCache {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
}

// pub mod basic;
pub mod egui;

pub struct HardwareMaterialShaderResource {
    pub pass: smallvec::SmallVec<[PipelinePass; 1]>,
}

#[derive(Default)]
pub struct HardwareMaterialShaderCache {
    map: HashMap<(String, u64), HardwareMaterialShaderResource>,
    shader_module_map: HashMap<String, (wgpu::ShaderModule, spirq::SpirvBinary)>,
}

impl HardwareMaterialShaderCache {
    pub fn resolve<S: Into<String>, F: FnOnce(PipelineReflector) -> PipelineReflector>(
        &mut self,
        name: S,
        id: u64,
        gpu: &WGPUResource,
        f: F,
    ) {
        let name = name.into();
        let full_name = format!("compile_shaders/{}", name);
        let mut reflector = PipelineReflector::new(None, gpu.device());
        let file = std::fs::File::open(format!("{}.vert", full_name)).unwrap();
        let mut r = BufReader::new(file);
        let mut data = Vec::new();

        r.read_to_end(&mut data).unwrap();
        let vs = wgpu::util::make_spirv(&data);
        let vs = wgpu::ShaderModuleDescriptor {
            label: None,
            source: vs,
        };
        reflector = reflector.add_vs(vs);
        data.clear();

        let file = std::fs::File::open(format!("{}.frag", full_name)).unwrap();
        let mut r = BufReader::new(file);

        r.read_to_end(&mut data).unwrap();
        let fs = wgpu::util::make_spirv(&data);
        let fs = wgpu::ShaderModuleDescriptor {
            label: None,
            source: fs,
        };
        reflector = reflector.add_fs(fs);
        reflector = f(reflector);

        let rp = reflector.build().unwrap();
        let mut pass = smallvec::SmallVec::new();
        pass.push(rp);

        self.map
            .insert((name, id), HardwareMaterialShaderResource { pass });
    }

    pub fn get(&self, name: &str, id: u64) -> &HardwareMaterialShaderResource {
        self.map.get(&(name.to_owned(), id)).unwrap()
    }
}

pub type MaterialResourceId = u64;
