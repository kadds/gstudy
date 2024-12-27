use std::{
    collections::HashMap,
    sync::Arc,
};

use itertools::Itertools;
use tshader::{tech::GlobalVariable, Pass};

#[derive(Debug)]
enum PipelineStateObjectInner {
    Render(wgpu::RenderPipeline),
    Compute(wgpu::ComputePipeline),
}

pub struct Variable {
    size: u32,
}

#[derive(Debug, Default)]
pub struct Uniforms {
    pub vars: HashMap<String, GlobalVariable>,
    pub group: u32,
}

#[derive(Debug)]
pub struct PipelineStateObject {
    inner: PipelineStateObjectInner,
    global_variables: HashMap<BindGroupType, Uniforms>,
    name: String,
}

#[derive(Debug, Hash, PartialEq, Eq, Copy, Clone)]
pub enum BindGroupType {
    Camera,
    Material,
    Light,
    Shadow,
    Object,
}

impl PipelineStateObject {
    pub fn pass_name(&self) -> &str {
        &self.name
    }
    pub fn render(&self) -> &wgpu::RenderPipeline {
        match &self.inner {
            PipelineStateObjectInner::Render(r) => &r,
            _ => panic!("unsupported pipeline type"),
        }
    }

    pub fn get_bind_group_layout(
        &self,
        ty: BindGroupType,
    ) -> Option<(wgpu::BindGroupLayout, &Uniforms)> {
        match &self.inner {
            PipelineStateObjectInner::Render(r) => {
                let uniforms = self.global_variables.get(&ty)?;
                Some((r.get_bind_group_layout(uniforms.group), uniforms))
            }
            PipelineStateObjectInner::Compute(c) => {
                let uniforms = self.global_variables.get(&ty)?;
                Some((c.get_bind_group_layout(uniforms.group), uniforms))
            }
        }
    }
}

pub type MaterialPipelineStateObjects = Vec<Arc<PipelineStateObject>>;
pub type MaterialPipelineStateObjectsRef<'a> = &'a [Arc<PipelineStateObject>];

pub trait PipelineStateObjectCache {
    fn get(&self, id: u64, pass: Arc<Pass>) -> Arc<PipelineStateObject>;
    fn load(&self, device: &wgpu::Device, id: u64, pass: Arc<Pass>, rdo: RenderDescriptorObject);
}

pub mod immediate_pso;

fn find_visibility(pass: &Pass, name: &str) -> wgpu::ShaderStages {
    let mut visibility = wgpu::ShaderStages::empty();
    if let Some(vs) = &pass.vs {
        if vs.global_reference.contains(name) {
            visibility.insert(wgpu::ShaderStages::VERTEX);
        }
    }
    if let Some(fs) = &pass.fs {
        if fs.global_reference.contains(name) {
            visibility.insert(wgpu::ShaderStages::FRAGMENT);
        }
    }
    if let Some(cs) = &pass.cs {
        if cs.global_reference.contains(name) {
            visibility.insert(wgpu::ShaderStages::COMPUTE);
        }
    }
    visibility
}

fn build_bind_group_layout_entry(
    device: &wgpu::Device,
    list: &[&str],
    pass: &Pass,
) -> anyhow::Result<(wgpu::BindGroupLayout, HashMap<String, GlobalVariable>)> {
    let mut entries = vec![];
    let mut vars: HashMap<String, GlobalVariable> = HashMap::new();

    for name in list {
        let variable = pass.global_variables.get(*name).unwrap();
        let visibility = find_visibility(pass, name);
        vars.insert(name.to_string(), variable.clone());

        match variable {
            tshader::tech::GlobalVariable::Struct(s) => {
                entries.push(wgpu::BindGroupLayoutEntry {
                    visibility,
                    binding: s.binding,
                    count: None,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: None,
                    },
                });
            }
            tshader::tech::GlobalVariable::Texture(t) => {
                entries.push(wgpu::BindGroupLayoutEntry {
                    visibility,
                    binding: t.binding,
                    count: None,
                    ty: wgpu::BindingType::Texture {
                        sample_type: t.sample_type,
                        view_dimension: t.dimension,
                        multisampled: t.multisampled,
                    },
                });
            }
            tshader::tech::GlobalVariable::Sampler(s) => {
                let ty = if s.comparison {
                    wgpu::SamplerBindingType::Comparison
                } else {
                    wgpu::SamplerBindingType::Filtering
                };
                entries.push(wgpu::BindGroupLayoutEntry {
                    visibility,
                    binding: s.binding,
                    count: None,
                    ty: wgpu::BindingType::Sampler(ty),
                });
            }
            _ => anyhow::bail!("unsupported global variable in this group"),
        }
    }

    let desc = &wgpu::BindGroupLayoutDescriptor {
        label: None,
        entries: &entries,
    };
    let res = device.create_bind_group_layout(desc);
    Ok((res, vars))
}

fn create_compute_pipeline(
    device: &wgpu::Device,
    pass: &Pass,
    pipeline_layout: &wgpu::PipelineLayout,
) -> anyhow::Result<Arc<PipelineStateObject>> {
    //     compute pipeline
    let desc = wgpu::ComputePipelineDescriptor {
        label: Some(&pass.name),
        layout: Some(&pipeline_layout),
        module: &pass.cs.as_ref().unwrap().device_module,
        entry_point: Some("cs_main"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    };
    let pipeline = device.create_compute_pipeline(&desc);
    Ok(Arc::new(PipelineStateObject {
        inner: PipelineStateObjectInner::Compute(pipeline),
        global_variables: HashMap::default(),
        name: pass.name.clone(),
    }))
}

fn create_render_pipeline(
    device: &wgpu::Device,
    pass: &Pass,
    pipeline_layout: &wgpu::PipelineLayout,
    rdo: &RenderDescriptorObject,
    global_variables: HashMap<BindGroupType, Uniforms>,
) -> anyhow::Result<Arc<PipelineStateObject>> {
    let mut vertex_buffer_layouts = Vec::new();
    let mut default_layouts = vec![];
    let mut main_layouts = vec![];

    if rdo.vertex_split_slot {
        let mut offset = 0;
        let mut position_size = 0;
        for (name, binding) in &pass.local_variables {
            if binding.binding == 0 {
                default_layouts.push(wgpu::VertexAttribute {
                    format: binding.format,
                    offset: 0,
                    shader_location: 0,
                });
                position_size = binding.size as u64;
            } else {
                main_layouts.push(wgpu::VertexAttribute {
                    format: binding.format,
                    offset,
                    shader_location: binding.binding,
                });
                offset += binding.size as u64;
            }
        }
        vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
            array_stride: position_size,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &default_layouts,
        });
        if !main_layouts.is_empty() {
            vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
                array_stride: offset,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &main_layouts,
            });
        }
    } else {
        let mut offset = 0;
        let mut max_alignment = 0 as u64;
        let local_variables_sort_by_binding: Vec<_> = pass.local_variables.iter().sorted_by(|a, b| {
            a.1.binding.cmp(&b.1.binding)
        }).collect();

        for (name, binding) in &local_variables_sort_by_binding {
            main_layouts.push(wgpu::VertexAttribute {
                format: binding.format,
                offset,
                shader_location: binding.binding,
            });
            offset += binding.size as u64;
            max_alignment = max_alignment.max(binding.size as u64);
        }
        let stride = offset;

        vertex_buffer_layouts.push(wgpu::VertexBufferLayout {
            array_stride: stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &main_layouts,
        });
    }

    let mut desc = wgpu::RenderPipelineDescriptor {
        label: Some(&pass.name),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: &pass.vs.as_ref().unwrap().device_module,
            entry_point: Some("vs_main"),
            buffers: &vertex_buffer_layouts,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        primitive: rdo.primitive,
        depth_stencil: rdo.depth.clone(),
        multisample: rdo.multi_sample,
        fragment: None,
        multiview: None,
        cache: None,
    };
    if let Some(fs) = &pass.fs {
        desc.fragment = Some(wgpu::FragmentState {
            module: &fs.device_module,
            entry_point: Some("fs_main"),
            targets: &rdo.color_targets,
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        })
    }
    let pipeline = device.create_render_pipeline(&desc);

    Ok(Arc::new(PipelineStateObject {
        inner: PipelineStateObjectInner::Render(pipeline),
        global_variables,
        name: pass.name.clone(),
    }))
}

pub(crate) fn load_pso(
    device: &wgpu::Device,
    pass: Arc<Pass>,
    rdo: &RenderDescriptorObject,
) -> anyhow::Result<Arc<PipelineStateObject>> {
    let mut layouts: Vec<wgpu::BindGroupLayout> = Vec::new();
    let mut group_name_to_enum = HashMap::new();
    group_name_to_enum.insert("MaterialUniform", BindGroupType::Material);
    group_name_to_enum.insert("CameraUniform", BindGroupType::Camera);
    group_name_to_enum.insert("ShadowUniform", BindGroupType::Shadow);
    group_name_to_enum.insert("LightUniform", BindGroupType::Light);
    group_name_to_enum.insert("ObjectUniform", BindGroupType::Object);
    let mut available_groups = HashMap::<_, Vec<&str>>::new();

    for (name, var) in &pass.global_variables {
        match var {
            tshader::tech::GlobalVariable::Struct(s) => {
                let bind_group_type = group_name_to_enum.get(s.group_name.as_str()).unwrap();
                available_groups
                    .entry(*bind_group_type)
                    .or_default()
                    .push(name.as_str());
            }
            tshader::tech::GlobalVariable::Texture(t) => {
                let bind_group_type = group_name_to_enum.get(&t.group_name.as_str()).unwrap();
                available_groups
                    .entry(*bind_group_type)
                    .or_default()
                    .push(name.as_str());
            }
            tshader::tech::GlobalVariable::Sampler(s) => {
                let bind_group_type = group_name_to_enum.get(&s.group_name.as_str()).unwrap();
                available_groups
                    .entry(*bind_group_type)
                    .or_default()
                    .push(name.as_str());
            }
            tshader::tech::GlobalVariable::PushConstant(c) => {
                let bind_group_type = group_name_to_enum.get(&c.group_name.as_str()).unwrap();
                available_groups
                    .entry(*bind_group_type)
                    .or_default()
                    .push(name.as_str());
            }
        }
    }

    // camera bind group
    let mut global_variables: HashMap<BindGroupType, Uniforms> = HashMap::new();

    if let Some(list) = available_groups.get(&BindGroupType::Camera) {
        let (layout, vars) = build_bind_group_layout_entry(device, &list, &pass)?;
        layouts.push(layout);
        let s = global_variables.entry(BindGroupType::Camera).or_default();
        s.group = (layouts.len() - 1) as u32;
        s.vars = vars;
    }
    // material bind group
    if let Some(list) = available_groups.get(&BindGroupType::Material) {
        let (layout, vars) = build_bind_group_layout_entry(device, &list, &pass)?;
        layouts.push(layout);
        let s = global_variables.entry(BindGroupType::Material).or_default();
        s.group = (layouts.len() - 1) as u32;
        s.vars = vars;
    }

    // shadow bind group
    if let Some(list) = available_groups.get(&BindGroupType::Light) {
        let (layout, vars) = build_bind_group_layout_entry(device, &list, &pass)?;
        layouts.push(layout);
        let s = global_variables.entry(BindGroupType::Light).or_default();
        s.group = (layouts.len() - 1) as u32;
        s.vars = vars;
    }

    // shadow bind group
    if let Some(list) = available_groups.get(&BindGroupType::Shadow) {
        let (layout, vars) = build_bind_group_layout_entry(device, &list, &pass)?;
        layouts.push(layout);
        let s = global_variables.entry(BindGroupType::Shadow).or_default();
        s.group = (layouts.len() - 1) as u32;
        s.vars = vars;
    }

    // object bind group
    let mut constants = vec![];

    if let Some(list) = available_groups.get(&BindGroupType::Object) {
        for name in list {
            let var = pass.global_variables.get(*name).unwrap();
            if let tshader::tech::GlobalVariable::PushConstant(c) = var {
                constants.push(wgpu::PushConstantRange {
                    stages: find_visibility(&pass, name),
                    range: 0..(c.size as u32),
                });
            }
        }
    }

    let mut ref_layouts = Vec::new();
    for layout in &layouts {
        ref_layouts.push(layout);
    }

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&pass.name),
        bind_group_layouts: &ref_layouts,
        push_constant_ranges: &constants,
    });

    

    if pass.cs.is_some() {
        create_compute_pipeline(device, &pass, &pipeline_layout)
    } else {
        create_render_pipeline(device, &pass, &pipeline_layout, rdo, global_variables)
    }
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
    vertex_split_slot: bool,
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
            vertex_split_slot: true,
            // constant_stages: vec![],
            // global_bind_group_layout: None,
        }
    }

    pub fn set_msaa(mut self, c: u32) -> Self {
        self.multi_sample.count = c;
        self
    }

    pub fn vertex_no_split(mut self) -> Self {
        self.vertex_split_slot = false;
        self
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
