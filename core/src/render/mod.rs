use indexmap::IndexMap;
use material::RenderMaterialPsoBuilder;
use pso::PipelineStateObjectCache;
use std::{
    any::TypeId,
    collections::{BTreeMap, HashMap},
    sync::Arc,
};
use tech::ShaderTechCollection;

use crate::{
    backends::wgpu_backend::WGPUResource,
    graph::rdg::{backend::GraphBackend, RenderGraph, RenderGraphBuilder},
    material::{basic::BasicMaterialFace, MaterialArc},
    render::material::{RenderSourceIndirectObjects, RenderSourceLayer, SetupResource},
    scene::{layer_str, LayerId, Scene, LAYER_UI},
    types::{Mat4x4f, Vec4f},
    util::any_as_u8_slice,
};


use self::material::{basic::BasicMaterialRendererFactory, MaterialRendererFactory};
use self::material::{RenderMaterialContext, RenderSource};

pub struct RenderParameter<'a> {
    pub gpu: Arc<WGPUResource>,
    pub scene: Arc<Scene>,
    pub g: &'a mut RenderGraph,
}

pub struct SetupConfig {
    pub msaa: u32,
}

pub trait ModuleRenderer {
    fn setup(
        &mut self,
        g: &mut RenderGraphBuilder,
        gpu: Arc<WGPUResource>,
        scene: &Scene,
        config: &SetupConfig,
    );
    fn render(&mut self, parameter: RenderParameter);
    fn stop(&mut self);
}

pub mod attachment;
pub mod collector;
pub mod common;
pub mod collection;
pub mod material;
pub mod pso;
pub mod tech;

#[repr(C)]
struct GlobalUniform3d {
    mat: Mat4x4f,
    direction: Vec4f,
}

#[repr(C)]
struct GlobalUniform2d {
    size: Vec4f,
}

pub struct GlobalUniform {
    pub buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    pub bind_group_layout: Arc<wgpu::BindGroupLayout>,
}

impl GlobalUniform {
    pub fn new(gpu: &WGPUResource, layout: Arc<wgpu::BindGroupLayout>, size: u32) -> Self {
        let label = Some("global uniform");
        let buffer = gpu.new_uniform_buffer(label, size as u64);
        let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buffer,
                    offset: 0,
                    size: None,
                }),
            }],
        });
        Self {
            buffer,
            bind_group,
            bind_group_layout: layout,
        }
    }
}

struct HardwareRendererInner {
    main_camera: Arc<GlobalUniform>,
    ui_camera: Arc<GlobalUniform>,
}

pub struct HardwareRenderer {
    material_renderer_factory: HashMap<TypeId, Box<dyn MaterialRendererFactory>>,
    inner: Option<HardwareRendererInner>,
    shader_tech_collection: Arc<ShaderTechCollection>,
}

impl HardwareRenderer {
    #[profiling::function]
    pub fn new() -> Self {
        let mut material_renderer_factory =
            HashMap::<TypeId, Box<dyn MaterialRendererFactory>>::new();

        material_renderer_factory.insert(
            TypeId::of::<BasicMaterialFace>(),
            Box::<BasicMaterialRendererFactory>::default(),
        );
        let loader = tshader::default_shader_tech_loader();
        let pso_cache = pso::immediate_pso::ImmediatePipelineStateObjectCache::new();

        let shader_tech_collection =
            Arc::new(ShaderTechCollection::new(loader, Box::new(pso_cache)));

        Self {
            material_renderer_factory,
            inner: None,
            shader_tech_collection,
        }
    }

    pub fn add_factory(&mut self, face_id: TypeId, factory: Box<dyn MaterialRendererFactory>) {
        log::info!("install factory {:?}", face_id);
        self.material_renderer_factory.insert(face_id, factory);
    }

    fn setup_global_uniform(&mut self, gpu: &WGPUResource) {
        self.inner.get_or_insert_with(|| {
            let bind_layout = Arc::new(gpu.device().create_bind_group_layout(
                &wgpu::BindGroupLayoutDescriptor {
                    label: Some("global layout"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        count: None,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: None,
                        },
                    }],
                },
            ));
            let main = GlobalUniform::new(
                gpu,
                bind_layout.clone(),
                std::mem::size_of::<GlobalUniform3d>() as u32,
            );
            let ui = GlobalUniform::new(
                gpu,
                bind_layout.clone(),
                std::mem::size_of::<GlobalUniform2d>() as u32,
            );
            HardwareRendererInner {
                main_camera: Arc::new(main),
                ui_camera: Arc::new(ui),
            }
        });
    }
    fn copy_camera_uniform(&mut self, p: &RenderParameter) {
        let scene = p.scene.clone();

        // prepare camera uniform buffer
        let inner = self.inner.as_ref().unwrap();
        if let Some(camera) = scene.main_camera_ref() {
            let vp = camera.vp();
            let direction = (camera.to() - camera.from()).normalize();
            let data = GlobalUniform3d {
                mat: vp,
                direction: Vec4f::new(direction.x, direction.y, direction.z, 1.0f32),
            };
            p.gpu
                .queue()
                .write_buffer(&inner.main_camera.buffer, 0, any_as_u8_slice(&data));
        }
        let size = scene.ui_camera_ref().width_height();

        let data = GlobalUniform2d {
            size: Vec4f::new(size.x, size.y, 0f32, 0f32),
        };
        p.gpu
            .queue()
            .write_buffer(&inner.ui_camera.buffer, 0, any_as_u8_slice(&data));
    }
}

impl ModuleRenderer for HardwareRenderer {
    #[profiling::function]
    fn setup(
        &mut self,
        g: &mut RenderGraphBuilder,
        gpu: Arc<WGPUResource>,
        scene: &Scene,
        config: &SetupConfig,
    ) {
        log::info!("hardware setup");
        self.setup_global_uniform(&gpu);

        let inner = self.inner.as_ref().unwrap();

        let setup_resource = SetupResource {
            ui_camera: inner.ui_camera.clone(),
            main_camera: inner.main_camera.clone(),
            shader_tech_collection: self.shader_tech_collection.clone(),
            scene: scene,
            msaa: config.msaa,
        };
        let container = scene.get_container();

        // face map
        let mut material_map: IndexMap<TypeId, BTreeMap<LayerId, Vec<MaterialArc>>> =
            IndexMap::new();

        for (layer, sorter) in scene.layers() {
            let sort_objects = sorter.lock().unwrap().sort_and_cull();
            log::info!(
                "setup layer {} {} total {} object sort {:?}",
                layer,
                layer_str(layer),
                sort_objects.len(),
                sort_objects
            );

            for obj_id in sort_objects {
                let o = container.get(&obj_id).unwrap();
                let obj = o.o();
                let mat_face_id = obj.material_arc().face_id();
                material_map
                    .entry(mat_face_id)
                    .and_modify(|v| {
                        v.entry(layer)
                            .and_modify(|r| r.push(obj.material_arc()))
                            .or_insert_with(|| vec![obj.material_arc()]);
                    })
                    .or_insert_with(|| {
                        let mut m = BTreeMap::new();
                        m.insert(layer, vec![obj.material_arc()]);
                        m
                    });
            }
        }

        for (mat_face_id, materials) in material_map {
            let f = match self.material_renderer_factory.get(&mat_face_id) {
                Some(v) => v,
                None => {
                    log::error!(
                        "material {:?} renderer factory not exist, check your plugin list",
                        mat_face_id
                    );
                    continue;
                }
            };
            profiling::scope!("material setup", &format!("{:?}", mat_face_id));
            f.setup(
                &RenderMaterialPsoBuilder::new(materials),
                &gpu,
                g,
                &setup_resource,
            );
        }
    }

    #[profiling::function]
    fn render(&mut self, p: RenderParameter) {
        self.copy_camera_uniform(&p);

        let gpu = p.gpu.clone();
        let scene = p.scene;
        let storage = scene.get_container();

        let mut render_source_map: HashMap<TypeId, RenderSource> = HashMap::new();

        let inner = self.inner.as_ref().unwrap();

        for (layer, sorter) in scene.layers() {
            let main_camera = if layer >= LAYER_UI {
                inner.ui_camera.clone()
            } else {
                inner.main_camera.clone()
            };

            let sort_objects = sorter.lock().unwrap().sort_and_cull();
            log::info!(
                "layer {} {} total {} object sort {:?}",
                layer,
                layer_str(layer),
                sort_objects.len(),
                sort_objects
            );

            for obj_id in &sort_objects {
                let o = storage.get(obj_id).unwrap();
                let obj = o.o();
                let mat_id = obj.material_arc().id();
                let face_id = obj.material_arc().face_id();

                let rs = render_source_map
                    .entry(face_id)
                    .or_insert_with(|| RenderSource {
                        gpu: gpu.clone(),
                        scene: scene.clone(),
                        list: vec![],
                        layer_map_index: HashMap::new(),
                    });

                if let Some(rsl) = rs.list.last_mut() {
                    if rsl.layer == layer {
                        // append
                        let last_mat = rsl.material.last_mut().unwrap();
                        if last_mat.mat_id != mat_id {
                            rsl.material.push(RenderSourceIndirectObjects {
                                material: obj.material_arc(),
                                mat_id,
                                offset: rsl.objects.len(),
                                count: 1,
                            });
                            rsl.objects.push(*obj_id);
                        } else {
                            last_mat.count += 1;
                            rsl.objects.push(*obj_id);
                        }
                        rs.layer_map_index.insert(layer, rs.list.len() - 1);
                        continue;
                    }
                }
                // new list
                rs.list.push(RenderSourceLayer {
                    objects: vec![*obj_id],
                    material: vec![RenderSourceIndirectObjects {
                        material: obj.material_arc(),
                        mat_id,
                        offset: 0,
                        count: 1,
                    }],
                    main_camera: main_camera.clone(),
                    layer,
                });
                rs.layer_map_index.insert(layer, rs.list.len() - 1);
            }
        }
        log::debug!("{:?}", render_source_map);

        let rm_context = RenderMaterialContext {
            map: render_source_map,
        };
        let backend = GraphBackend::new(p.gpu.clone());
        p.g.execute(backend, &rm_context);
    }

    fn stop(&mut self) {}
}
