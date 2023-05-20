use std::{
    any::{Any, TypeId},
    collections::{HashMap, HashSet},
    ops::Range,
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::{
    backends::wgpu_backend::{self, WGPURenderTarget, WGPURenderer, WGPUResource},
    graph::rdg::{backend::GraphBackend, RenderGraph, RenderGraphBuilder},
    material::{basic::BasicMaterialFace, egui::EguiMaterialFace, MaterialFace, MaterialId},
    ps::{DepthDescriptor, PrimitiveStateDescriptor},
    scene::{Scene, LAYER_UI},
    types::{Mat4x4f, Rectu, Vec2f},
    util::any_as_u8_slice,
};

use self::material::{
    egui::{EguiMaterialHardwareRenderer, EguiMaterialRendererFactory},
    MaterialRendererFactory, MaterialResourceId,
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

struct GlobalUniform3d {
    mat: Mat4x4f,
}

struct GlobalUniform2d {
    size: Vec2f,
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

struct DrawCommands {
    commands: Vec<DrawCommand>,
    push_constant_buffer: Vec<u8>,
    bind_groups: Vec<Arc<wgpu::BindGroup>>,
    constant_buffer_size: u16,
    clips: Vec<Rectu>,
    bind_group_count: u8,
    pipelines: Vec<Arc<Pipeline>>,
    global_bind_group: u32,
}

struct DrawCommandBuilder<'a> {
    inner: &'a mut DrawCommands,
    command: Option<DrawCommand>,
}

impl<'a> DrawCommandBuilder<'a> {
    pub fn with_draw(mut self, index: Range<u64>, vertex: Range<u64>, draw_count: u32) -> Self {
        self.set_draw(index, vertex, draw_count);
        self
    }

    pub fn set_draw(&mut self, index: Range<u64>, vertex: Range<u64>, draw_count: u32) {
        self.command = Some(DrawCommand {
            vertex,
            index,
            draw_count,
            bind_groups: u32::MAX,
            pipeline: u32::MAX,
            push_constant_offset: u32::MAX,
            clip_offset: u32::MAX,
        })
    }

    pub fn with_clip(mut self, clip: Rectu) -> Self {
        if let Some(last) = self.inner.clips.last() {
            if *last == clip {
                self.command.as_mut().unwrap().clip_offset = (self.inner.clips.len() - 1) as u32;
                return self;
            }
        }
        self.command.as_mut().unwrap().clip_offset = (self.inner.clips.len()) as u32;
        self.inner.clips.push(clip);
        self
    }

    pub fn with_constant(mut self, buffer: &[u8]) -> Self {
        let offset = self.inner.push_constant_buffer.len() as u32;
        self.inner.push_constant_buffer.extend_from_slice(buffer);
        self.command.as_mut().unwrap().push_constant_offset = offset;

        self
    }

    pub fn with_bind_groups(mut self, group: &[Arc<wgpu::BindGroup>]) -> Self {
        assert_eq!(group.len() as u8, self.inner.bind_group_count);
        let offset = self.inner.bind_groups.len() as u32;
        self.inner.bind_groups.extend_from_slice(group);
        self.command.as_mut().unwrap().bind_groups = offset;

        self
    }

    pub fn with_pipeline(mut self, pipeline: Arc<Pipeline>) -> Self {
        let offset = self.inner.pipelines.len() as u32;
        self.inner.pipelines.push(pipeline);
        self.command.as_mut().unwrap().pipeline = offset;
        self
    }
    pub fn with_pipeline_offset(mut self, pipeline: u32) -> Self {
        self.command.as_mut().unwrap().pipeline = pipeline;
        self
    }

    pub fn build(self) {
        self.inner.commands.push(self.command.unwrap());
    }
}

impl DrawCommands {
    fn new(constant_buffer_size: usize, bind_group_count: usize) -> Self {
        let constant_buffer_size = constant_buffer_size as u16;
        let bind_group_count = bind_group_count as u8;
        Self {
            commands: Vec::new(),
            push_constant_buffer: Vec::new(),
            constant_buffer_size,
            bind_group_count,
            clips: Vec::new(),
            bind_groups: Vec::new(),
            pipelines: Vec::new(),
            global_bind_group: u32::MAX,
        }
    }
    pub fn builder<'a>(&'a mut self) -> DrawCommandBuilder<'a> {
        DrawCommandBuilder {
            inner: self,
            command: None,
        }
    }
    fn get_bind_groups(&self, command: &DrawCommand) -> impl Iterator<Item = &wgpu::BindGroup> {
        if command.bind_groups == u32::MAX {
            self.bind_groups[0..0].iter()
        } else {
            self.bind_groups[(command.bind_groups as usize)
                ..((command.bind_groups + self.bind_group_count as u32) as usize)]
                .iter()
        }
        .map(|v| v.as_ref())
    }

    fn get_constant_data(&self, command: &DrawCommand) -> &[u8] {
        &self.push_constant_buffer[(command.push_constant_offset as usize)
            ..((command.push_constant_offset + self.constant_buffer_size as u32) as usize)]
    }
    fn get_clip(&self, command: &DrawCommand) -> Option<Rectu> {
        if command.clip_offset == u32::MAX {
            return None;
        } else {
            Some(self.clips[command.clip_offset as usize])
        }
    }

    fn push_cached_pipeline(&mut self, pipeline: Arc<Pipeline>) -> u32 {
        let offset = self.pipelines.len() as u32;
        self.pipelines.push(pipeline);
        offset
    }

    fn set_global_bind_group(&mut self, bind_group: Arc<wgpu::BindGroup>) {
        if self.global_bind_group != u32::MAX {
            return;
        }
        self.global_bind_group = self.bind_groups.len() as u32;
        self.bind_groups.push(bind_group);
    }

    fn get_global_bind_group(&self) -> Option<&wgpu::BindGroup> {
        if self.global_bind_group != u32::MAX {
            Some(&self.bind_groups[self.global_bind_group as usize])
        } else {
            None
        }
    }

    fn get_pipeline(&self, command: &DrawCommand) -> &Pipeline {
        &self.pipelines[command.pipeline as usize]
    }

    pub fn commands(&mut self) -> Vec<DrawCommand> {
        let mut tmp = Vec::new();
        std::mem::swap(&mut self.commands, &mut tmp);
        tmp
    }

    pub fn clear(&mut self) {
        self.bind_groups.clear();
        self.clips.clear();
        self.pipelines.clear();
        self.global_bind_group = u32::MAX;
    }
}
struct DrawCommand {
    vertex: Range<u64>,
    index: Range<u64>,
    draw_count: u32,
    bind_groups: u32,
    pipeline: u32,
    push_constant_offset: u32,
    clip_offset: u32,
}

struct GlobalUniform {
    buffer: wgpu::Buffer,
    bind_group: Arc<wgpu::BindGroup>,
}

impl GlobalUniform {
    pub fn new(gpu: &WGPUResource, layout: &wgpu::BindGroupLayout, size: u32) -> Self {
        let label = Some("global uniform");
        let buffer = gpu.new_uniform_buffer(label, size as u64);
        let bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
            label,
            layout: layout,
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
            bind_group: Arc::new(bind_group),
        }
    }
}

struct HardwareRendererInner {
    main_camera: GlobalUniform,
    ui_camera: GlobalUniform,
}

pub struct HardwareRenderer {
    material_renderer_factory: HashMap<TypeId, Box<dyn MaterialRendererFactory>>,
    material_renderers: HashMap<PassIdent, Arc<Mutex<dyn MaterialRenderer>>>,
    shader_loader: tshader::Loader,
    inner: Option<HardwareRendererInner>,
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
        let shader_loader = tshader::Loader::new("./shaders/desc.toml".into()).unwrap();

        Self {
            material_renderer_factory,
            material_renderers,
            shader_loader,
            inner: None,
        }
    }
}

impl ModuleRenderer for HardwareRenderer {
    fn setup(&mut self, g: &mut RenderGraphBuilder, gpu: Arc<WGPUResource>, scene: &mut Scene) {
        log::info!("hardware setup");
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
                &gpu,
                &bind_layout,
                std::mem::size_of::<GlobalUniform3d>() as u32,
            );
            let ui = GlobalUniform::new(
                &gpu,
                &bind_layout,
                std::mem::size_of::<GlobalUniform2d>() as u32,
            );
            HardwareRendererInner {
                main_camera: main,
                ui_camera: ui,
            }
        });

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
                        self.material_renderers.insert(
                            ident,
                            f.setup(ident, &mats, &gpu, g, &mut self.shader_loader),
                        );
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
                self.material_renderers.insert(
                    ident,
                    f.setup(ident, &mats, &gpu, g, &mut self.shader_loader),
                );
            }
        }
    }

    fn render(&mut self, p: RenderParameter) {
        let gpu = p.gpu.clone();
        let scene = p.scene;

        // prepare camera uniform buffer
        let inner = self.inner.as_ref().unwrap();
        if let Some(camera) = scene.main_camera() {
            let vp = camera.vp();
            let data = GlobalUniform3d { mat: vp };
            p.gpu
                .queue()
                .write_buffer(&inner.main_camera.buffer, 0, any_as_u8_slice(&data));
        }
        if let Some(camera) = scene.ui_camera() {
            let size = camera.width_height();

            let data = GlobalUniform2d { size };
            p.gpu
                .queue()
                .write_buffer(&inner.ui_camera.buffer, 0, any_as_u8_slice(&data));
        }

        let g = p.g;

        let backend = GraphBackend::new(gpu);
        let mut encoder = backend.begin_thread();

        for (layer, objects) in scene.layers() {
            let camera_uniform = if *layer >= LAYER_UI {
                &inner.ui_camera.buffer
            } else {
                &inner.main_camera.buffer
            };

            for (skey, mat) in &objects.sorted_objects {
                let id = mat.face_id();
                let ident = PassIdent::new(id, *layer);
                let layer_objects = scene.layer(ident.layer);

                let objects = &layer_objects.map[&mat.id()];
                let mut ctx = MaterialRenderContext {
                    gpu: p.gpu.as_ref(),
                    scene: &scene,
                    camera_uniform,
                };
                let r = self.material_renderers.get(&ident).unwrap();
                let mut r = r.lock().unwrap();
                r.render_material(&mut ctx, objects, &mat, encoder.encoder_mut());
            }
        }
        drop(encoder);

        g.execute(|_, _| {}, |_| {}, backend);
    }

    fn stop(&mut self) {}
}

#[derive(Debug)]
enum Pipeline {
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
    inner: Arc<Vec<tshader::Pass>>,
    pass: Vec<Arc<Pipeline>>,
}

struct ColorTargetBuilder {
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
        self.target.blend = Some(wgpu::BlendState {
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
        });
        self
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
            layout_entries.push(entry.clone());
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
            push_constant_ranges: &[],
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
                    format: format.clone(),
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
        let pipeline = resolve_single_pass(gpu, &pass, &ins);
        desc.pass.push(Arc::new(pipeline));
    }

    desc
}
