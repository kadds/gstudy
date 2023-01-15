use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
};

use crate::{
    backends::wgpu_backend::{self, WGPURenderTarget, WGPURenderer, WGPUResource},
    graph::rdg::{backend::GraphBackend, RenderGraph, RenderGraphBuilder},
    material::{basic::BasicMaterialFace, egui::EguiMaterialFace, MaterialFace, MaterialId},
    scene::{camera::RenderAttachment, Scene},
    types::Mat4x4f,
};

use self::material::{
    egui::{EguiMaterialHardwareRenderer, EguiMaterialRendererFactory},
    HardwareMaterialShaderCache, MaterialRendererFactory, MaterialResourceId,
};
use self::material::{MaterialRenderContext, MaterialRenderer};

pub struct RenderParameter<'a> {
    pub gpu: Arc<WGPUResource>,
    pub scene: &'a mut Scene,
    pub g: &'a mut RenderGraph,
}

pub trait ModuleRenderer {
    fn setup(&mut self, g: &mut RenderGraphBuilder, gpu: Arc<WGPUResource>, scene: &mut Scene);
    fn render(&mut self, parameter: RenderParameter);
    fn stop(&mut self);
}

pub mod common;
mod material;

struct WVP {
    mat: Mat4x4f,
}
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct PassIdent {
    type_id: TypeId,
    layer: u64,
}

impl PassIdent {
    pub fn new(type_id: TypeId, layer: u64) -> Self {
        Self { type_id, layer }
    }

    pub fn new_from<T: MaterialFace>(layer: u64) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            layer,
        }
    }
}

pub struct HardwareRenderer {
    material_renderer_factory: HashMap<TypeId, Box<dyn MaterialRendererFactory>>,
    material_renderers: HashMap<PassIdent, Arc<dyn MaterialRenderer>>,
    cache: HardwareMaterialShaderCache,
}

struct DrawCommand {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    bind_groups: smallvec::SmallVec<[wgpu::BindGroup; 2]>,
}

enum RenderMethod {
    Forward,
    Deferred,
    ForwardPlus,
    DeferredPlus,
}

impl HardwareRenderer {
    pub fn new() -> Self {
        let mut material_renderer_factory =
            HashMap::<TypeId, Box<dyn MaterialRendererFactory>>::new();

        let material_renderers = HashMap::new();
        material_renderer_factory.insert(
            TypeId::of::<EguiMaterialFace>(),
            Box::new(EguiMaterialRendererFactory::default()),
        );

        Self {
            material_renderer_factory,
            material_renderers,
            cache: HardwareMaterialShaderCache::default(),
        }
    }
}

impl ModuleRenderer for HardwareRenderer {
    fn setup(&mut self, g: &mut RenderGraphBuilder, gpu: Arc<WGPUResource>, scene: &mut Scene) {
        log::info!("hardware setup");

        self.material_renderers.clear();

        scene.sort_all(|layer, m| {
            let f = self.material_renderer_factory.get(&m.face_id()).unwrap();
            f.sort_key(m, &gpu)
        });

        for (layer, objects) in scene.layers() {
            let mut last_material_id = TypeId::of::<u32>();
            let mut mats = Vec::new();
            let mut ident = PassIdent::new(last_material_id, *layer);

            for (_, mat) in &objects.sorted_objects {
                let id = mat.face_id();
                if last_material_id != id {
                    if !mats.is_empty() {
                        let f = self
                            .material_renderer_factory
                            .get(&last_material_id)
                            .unwrap();
                        self.material_renderers
                            .insert(ident, f.setup(ident, &mats, &gpu, g, &mut self.cache));
                    }
                    // new material face
                    last_material_id = id;
                    mats.clear();
                }
                mats.push(&mat);
                ident = PassIdent::new(last_material_id, *layer);
            }
            if !mats.is_empty() {
                let f = self
                    .material_renderer_factory
                    .get(&last_material_id)
                    .unwrap();
                self.material_renderers
                    .insert(ident, f.setup(ident, &mats, &gpu, g, &mut self.cache));
            }
        }
    }

    fn render(&mut self, p: RenderParameter) {
        let gpu = p.gpu.clone();
        let scene = p.scene;
        let g = p.g;

        let backend = GraphBackend::new(gpu);
        let mut encoder = backend.begin_thread();

        for (layer, objects) in scene.layers() {
            for (skey, mat) in &objects.sorted_objects {
                let id = mat.face_id();
                let ident = PassIdent::new(id, *layer);
                let layer_objects = scene.layer(ident.layer);

                let objects = &layer_objects.map[&mat.id()];
                let mut ctx = MaterialRenderContext {
                    gpu: p.gpu.as_ref(),
                    scene: &scene,
                    cache: &self.cache,
                };
                let r = self.material_renderers.get(&ident).unwrap();
                r.render_material(&mut ctx, objects, &mat, encoder.encoder_mut());
            }
        }
        drop(encoder);

        // for (ident, r) in &self.material_renderers {
        //     r.bind_render_resource(Box::new(|| {
        //         // let layer = scene.layer(ident.layer);
        //         // for (_, m) in layer.sorted_objects {
        //         //     if m.face_id() == ident.type_id {

        //         //     }
        //         // }
        //     }));
        // }

        g.execute(|_, _| {}, |_| {}, backend);
    }

    fn stop(&mut self) {}
}
