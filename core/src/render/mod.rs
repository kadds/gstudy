use indexmap::IndexMap;
use std::{any::TypeId, collections::HashMap, fmt::Debug, sync::Arc};

use crate::{
    backends::wgpu_backend::WGPUResource,
    graph::rdg::{backend::GraphBackend, RenderGraph, RenderGraphBuilder},
    material::{basic::BasicMaterialFace, Material},
    render::material::{RenderSourceIndirectObjects, RenderSourceLayer, SetupResource},
    scene::{layer_str, Scene, LAYER_UI},
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

pub trait ModuleRenderer {
    fn setup(&mut self, g: &mut RenderGraphBuilder, gpu: Arc<WGPUResource>, scene: &Scene);
    fn render(&mut self, parameter: RenderParameter);
    fn stop(&mut self);
}

pub mod common;
pub mod material;

struct GlobalUniform3d {
    mat: Mat4x4f,
}

struct GlobalUniform2d {
    size: Vec4f,
}

struct GlobalUniform {
    buffer: Arc<wgpu::Buffer>,
}

impl GlobalUniform {
    pub fn new(gpu: &WGPUResource, layout: &wgpu::BindGroupLayout, size: u32) -> Self {
        let label = Some("global uniform");
        let buffer = Arc::new(gpu.new_uniform_buffer(label, size as u64));
        Self { buffer }
    }
}

struct HardwareRendererInner {
    main_camera: GlobalUniform,
    ui_camera: GlobalUniform,
}

pub struct HardwareRenderer {
    material_renderer_factory: HashMap<TypeId, Box<dyn MaterialRendererFactory>>,
    shader_loader: tshader::Loader,
    inner: Option<HardwareRendererInner>,
}

impl HardwareRenderer {
    pub fn new() -> Self {
        let mut material_renderer_factory =
            HashMap::<TypeId, Box<dyn MaterialRendererFactory>>::new();

        material_renderer_factory.insert(
            TypeId::of::<BasicMaterialFace>(),
            Box::<BasicMaterialRendererFactory>::default(),
        );
        let shader_loader = tshader::Loader::new("./shaders/desc.toml".into()).unwrap();

        Self {
            material_renderer_factory,
            shader_loader,
            inner: None,
        }
    }

    pub fn add_factory(&mut self, face_id: TypeId, factory: Box<dyn MaterialRendererFactory>) {
        log::info!("install factory {:?}", face_id);
        self.material_renderer_factory.insert(face_id, factory);
    }

    fn setup_global_uniform(&mut self, gpu: &WGPUResource) {
        self.inner.get_or_insert_with(|| {
            let bind_layout =
                gpu.device()
                    .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                        label: Some("global layout"),
                        entries: &[wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            count: None,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                        }],
                    });
            let main = GlobalUniform::new(
                gpu,
                &bind_layout,
                std::mem::size_of::<GlobalUniform3d>() as u32,
            );
            let ui = GlobalUniform::new(
                gpu,
                &bind_layout,
                std::mem::size_of::<GlobalUniform2d>() as u32,
            );
            HardwareRendererInner {
                main_camera: main,
                ui_camera: ui,
            }
        });
    }
    fn copy_camera_uniform(&mut self, p: &RenderParameter) {
        let gpu = p.gpu.clone();
        let scene = p.scene.clone();

        // prepare camera uniform buffer
        let inner = self.inner.as_ref().unwrap();
        if let Some(camera) = scene.main_camera_ref() {
            let vp = camera.vp();
            let data = GlobalUniform3d { mat: vp };
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
    fn setup(&mut self, g: &mut RenderGraphBuilder, gpu: Arc<WGPUResource>, scene: &Scene) {
        log::info!("hardware setup");
        self.setup_global_uniform(&gpu);

        let inner = self.inner.as_ref().unwrap();

        let setup_resource = SetupResource {
            ui_camera: &inner.ui_camera.buffer,
            main_camera: &inner.main_camera.buffer,
            shader_loader: &self.shader_loader,
        };
        let container = scene.get_container();

        // face map
        let mut material_map: IndexMap<TypeId, Vec<Arc<Material>>> = IndexMap::new();

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
                let mat_face_id = obj.material().face_id();
                material_map
                    .entry(mat_face_id)
                    .and_modify(|v| v.push(obj.material_arc()))
                    .or_insert_with(|| vec![obj.material_arc()]);
            }
        }

        for (mat_face_id, materials) in material_map {
            let f = match self.material_renderer_factory.get(&mat_face_id) {
                Some(v) => v,
                None => {
                    log::error!("material {:?} renderer factory not exist", mat_face_id);
                    continue;
                }
            };
            f.setup(&materials, &gpu, g, &setup_resource);
        }
    }

    fn render(&mut self, p: RenderParameter) {
        self.copy_camera_uniform(&p);

        let gpu = p.gpu.clone();
        let scene = p.scene;
        let storage = scene.get_container();

        let mut render_source_map: HashMap<TypeId, RenderSource> = HashMap::new();

        let inner = self.inner.as_ref().unwrap();

        for (layer, sorter) in scene.layers() {
            let main_camera = if layer >= LAYER_UI {
                inner.ui_camera.buffer.clone()
            } else {
                inner.main_camera.buffer.clone()
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
                let mat_id = obj.material().id();
                let face_id = obj.material().face_id();

                let rs = render_source_map
                    .entry(face_id)
                    .or_insert_with(|| RenderSource {
                        gpu: gpu.clone(),
                        scene: scene.clone(),
                        list: vec![],
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
                })
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

#[derive(Debug)]
pub enum Pipeline {
    Render(wgpu::RenderPipeline),
    Compute(wgpu::ComputePipeline),
}

impl Pipeline {
    pub fn render(&self) -> &wgpu::RenderPipeline {
        match self {
            Pipeline::Render(r) => r,
            _ => panic!("unsupported pipeline type"),
        }
    }
    pub fn get_bind_group_layout(&self, index: u32) -> wgpu::BindGroupLayout {
        match self {
            Pipeline::Render(r) => r.get_bind_group_layout(index),
            Pipeline::Compute(c) => c.get_bind_group_layout(index),
        }
    }
}

#[derive(Debug)]
pub struct PipelinePassResource {
    #[allow(unused)]
    pub inner: Arc<Vec<tshader::Pass>>,
    pub pass: Vec<Arc<Pipeline>>,
}

pub struct ColorTargetBuilder {
    target: wgpu::ColorTargetState,
}

impl ColorTargetBuilder {
    pub fn new(format: wgpu::TextureFormat) -> Self {
        Self {
            target: wgpu::ColorTargetState {
                format,
                blend: None,
                write_mask: wgpu::ColorWrites::all(),
            },
        }
    }

    pub fn build(self) -> wgpu::ColorTargetState {
        self.target
    }

    pub fn set_append_blender(mut self) -> Self {
        self.target.blend = Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        });
        self
    }

    pub fn set_default_blender(mut self) -> Self {
        self.target.blend = Some(default_blender());
        self
    }

    pub fn set_blender(mut self, blender: wgpu::BlendState) -> Self {
        self.target.blend = Some(blender);
        self
    }

    pub fn clear_blender(mut self) -> Self {
        self.target.blend = None;
        self
    }
}

pub fn default_blender() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::SrcAlpha,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::Zero,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

pub struct RenderDescriptorObject {
    depth: Option<wgpu::DepthStencilState>,
    primitive: wgpu::PrimitiveState,
    multi_sample: wgpu::MultisampleState,
    color_targets: Vec<Option<wgpu::ColorTargetState>>,
}

impl RenderDescriptorObject {
    pub fn new() -> Self {
        Self {
            depth: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            multi_sample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            color_targets: vec![],
        }
    }

    pub fn add_target(mut self, target: wgpu::ColorTargetState) -> Self {
        self.color_targets.push(Some(target));
        self
    }

    pub fn add_empty_target(mut self) -> Self {
        self.color_targets.push(None);
        self
    }

    pub fn set_depth<F: FnOnce(&mut wgpu::DepthStencilState)>(
        mut self,
        format: wgpu::TextureFormat,
        f: F,
    ) -> Self {
        let mut depth = wgpu::DepthStencilState {
            format,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::Always,
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState::default(),
                back: wgpu::StencilFaceState::default(),
                read_mask: 0x0,
                write_mask: 0x0,
            },
            bias: wgpu::DepthBiasState::default(),
        };
        f(&mut depth);
        self.depth = Some(depth);
        self
    }

    pub fn set_primitive<F: FnOnce(&mut wgpu::PrimitiveState)>(mut self, f: F) -> Self {
        f(&mut self.primitive);
        self
    }
}

fn resolve_single_pass(
    gpu: &WGPUResource,
    pass: &tshader::Pass,
    ins: &RenderDescriptorObject,
) -> Pipeline {
    let mut layouts = Vec::new();

    {
        let mut layout_entries = Vec::new();
        let mut current = (u32::MAX, u32::MAX);
        for (pos, entry) in &pass.bind_layout {
            if current.0 != pos.group {
                if !layout_entries.is_empty() {
                    let layout =
                        gpu.device()
                            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                                label: Some(&pass.name),
                                entries: &layout_entries,
                            });
                    layouts.push(layout);
                    layout_entries.clear();
                }
            }
            current = (pos.group, pos.binding);
            layout_entries.push(*entry);
        }
        if !layout_entries.is_empty() {
            let layout = gpu
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some(&pass.name),
                    entries: &layout_entries,
                });
            layouts.push(layout);
        }
    }

    let mut ref_layouts = Vec::new();
    for layout in &layouts {
        ref_layouts.push(layout);
    }
    let pipeline_layout = gpu
        .device()
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&pass.name),
            bind_group_layouts: &ref_layouts,
            push_constant_ranges: &pass.constants,
        });

    if let Some(cs) = &pass.cs {
        let desc = wgpu::ComputePipelineDescriptor {
            label: Some(&pass.name),
            layout: Some(&pipeline_layout),
            module: &cs.device_module,
            entry_point: "cs_main",
        };
        let pipeline = gpu.device().create_compute_pipeline(&desc);
        Pipeline::Compute(pipeline)
    } else {
        // build vertex buffer layout firstly
        let mut vertex_buffer_layouts = Vec::new();
        let mut vertex_attrs = Vec::new();
        {
            let mut ranges_size = Vec::new();
            let mut current = (0, 0);
            let mut offset = 0;

            for (pos, format) in &pass.input_layout {
                if current.0 != pos.group {
                    if current.1 < vertex_attrs.len() {
                        ranges_size.push((current.1..vertex_attrs.len(), offset));
                    }
                    offset = 0;
                    current = (pos.group, vertex_attrs.len());
                }
                vertex_attrs.push(wgpu::VertexAttribute {
                    format: *format,
                    offset,
                    shader_location: pos.binding,
                });
                offset += format.size();
            }
            if current.1 < vertex_attrs.len() {
                ranges_size.push((current.1..vertex_attrs.len(), offset));
            }
            for (range, size) in ranges_size {
                vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
                    array_stride: size as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &vertex_attrs[range],
                });
            }
        }

        let mut desc = wgpu::RenderPipelineDescriptor {
            label: Some(&pass.name),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &pass.vs.as_ref().unwrap().device_module,
                entry_point: "vs_main",
                buffers: &vertex_buffer_layouts,
            },
            primitive: ins.primitive,
            depth_stencil: ins.depth.clone(),
            multisample: ins.multi_sample,
            fragment: None,
            multiview: None,
        };
        if let Some(fs) = &pass.fs {
            desc.fragment = Some(wgpu::FragmentState {
                module: &fs.device_module,
                entry_point: "fs_main",
                targets: &ins.color_targets,
            })
        }
        let pipeline = gpu.device().create_render_pipeline(&desc);
        Pipeline::Render(pipeline)
    }
}

pub fn resolve_pipeline(
    gpu: &WGPUResource,
    template: Arc<Vec<tshader::Pass>>,
    ins: RenderDescriptorObject,
) -> PipelinePassResource {
    let mut desc = PipelinePassResource {
        inner: template.clone(),
        pass: vec![],
    };

    for pass in template.iter() {
        let pipeline = resolve_single_pass(gpu, pass, &ins);
        desc.pass.push(Arc::new(pipeline));
    }

    desc
}
