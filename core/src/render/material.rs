use std::{
    any::{Provider, TypeId},
    collections::HashMap,
    io::{BufReader, Read},
    sync::{Arc, Mutex},
};

use crate::{
    backends::wgpu_backend::{PipelineReflector, WGPUResource},
    graph::rdg::{RenderGraph, RenderGraphBuilder},
    material::Material,
    scene::{Camera, Scene},
};

use super::PassIdent;

pub struct MaterialRenderContext<'a> {
    pub gpu: &'a WGPUResource,
    pub scene: &'a Scene,
    pub cache: &'a HardwareMaterialShaderCache,
    pub global_uniform: &'a wgpu::Buffer,
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

pub trait MaterialRendererFactory {
    fn setup(
        &self,
        pass_ident: PassIdent,
        material: &[&Material],
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        cache: &mut HardwareMaterialShaderCache,
    ) -> Arc<Mutex<dyn MaterialRenderer>>;
    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64;
}

struct BufferCache {
    vertex: wgpu::Buffer,
    index: wgpu::Buffer,
}

// pub mod basic;
pub mod egui;

pub struct HardwareMaterialShaderResource {
    pub pass: smallvec::SmallVec<[Arc<wgpu::RenderPipeline>; 1]>,
}

pub struct ShaderResource {
    model: wgpu::ShaderModule,
    binary: Vec<u8>,
}

pub struct ShadersResource {
    vs: Option<ShaderResource>,
    fs: Option<ShaderResource>,
    cs: Option<ShaderResource>,
}

#[derive(Default)]
pub struct HardwareMaterialShaderCache {
    map: HashMap<(String, u64), HardwareMaterialShaderResource>,
    shader_module_map: HashMap<String, ShadersResource>,
}

impl HardwareMaterialShaderCache {
    fn resolve_shader(&mut self, path: &str, gpu: &WGPUResource) {
        if self.shader_module_map.contains_key(path) {
            return;
        }
        let file = std::fs::File::open(format!("{}.vert", path)).unwrap();
        let mut r = BufReader::new(file);
        let mut data = Vec::new();

        r.read_to_end(&mut data).unwrap();
        let vss = wgpu::util::make_spirv(&data);
        let vs = wgpu::ShaderModuleDescriptor {
            label: None,
            source: vss,
        };
        let vs = gpu.device().create_shader_module(vs);

        let mut data2 = Vec::new();
        let file = std::fs::File::open(format!("{}.frag", path)).unwrap();
        let mut r = BufReader::new(file);

        r.read_to_end(&mut data2).unwrap();
        let fss = wgpu::util::make_spirv(&data2);
        let fs = wgpu::ShaderModuleDescriptor {
            label: None,
            source: fss,
        };
        let fs = gpu.device().create_shader_module(fs);
        let sr = ShadersResource {
            vs: Some(ShaderResource {
                model: vs,
                binary: data,
            }),
            fs: Some(ShaderResource {
                model: fs,
                binary: data2,
            }),
            cs: None,
        };

        self.shader_module_map.insert(path.to_owned(), sr);
    }

    pub fn resolve<S: Into<String>, F: FnOnce(PipelineReflector) -> PipelineReflector>(
        &mut self,
        name: S,
        id: u64,
        gpu: &WGPUResource,
        f: F,
    ) {
        let name = name.into();
        if self.map.contains_key(&(name.to_owned(), id)) {
            return;
        }
        let full_name = format!("compile_shaders/{}", name);

        self.resolve_shader(&full_name, gpu);

        let sr = self.shader_module_map.get(&full_name).unwrap();

        let mut reflector = PipelineReflector::new(None, gpu.device());
        if let Some(vs) = &sr.vs {
            reflector = reflector.add_vs2(&vs.model, &vs.binary);
        }
        if let Some(fs) = &sr.fs {
            reflector = reflector.add_fs2(&fs.model, &fs.binary);
        }
        reflector = f(reflector);

        let rp = reflector.build().unwrap();
        let mut pass = smallvec::SmallVec::new();
        pass.push(Arc::new(rp));

        self.map
            .insert((name, id), HardwareMaterialShaderResource { pass });
    }

    pub fn get(&self, name: &str, id: u64) -> &HardwareMaterialShaderResource {
        self.map.get(&(name.to_owned(), id)).unwrap()
    }
}

pub type MaterialResourceId = u64;
