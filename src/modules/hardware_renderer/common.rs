use std::collections::{BTreeMap, HashMap};

use spirq::{ty::ImageArrangement, EntryPoint, Locator, ReflectConfig, SpirvBinary, Variable};
use wgpu::*;

#[allow(dead_code)]
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum ShaderType {
    Vertex,
    Fragment,
    Compute,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub set: u32,
    pub binding: u32,
}

impl Position {
    pub fn new(set: u32, binding: u32) -> Self {
        Self { set, binding }
    }
}

#[derive(Debug)]
pub struct PipelinePass {
    pub pipeline: RenderPipeline,
    pub bind_group_layouts: Vec<BindGroupLayout>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct PipelineReflector<'a> {
    device: &'a Device,
    label: Option<&'static str>,
    vs: Option<ShaderModule>,
    fs: Option<(ShaderModule, FsTarget)>,
    cs: Option<ShaderModule>,
    vertex_attrs: BTreeMap<Position, VertexFormat>,
    bind_group_layout_entries: BTreeMap<Position, BindGroupLayoutEntry>,
}

fn make_reflection(shader: &ShaderModuleDescriptor) -> SpirvBinary {
    match &shader.source {
        ShaderSource::SpirV(val) => val.as_ref().into(),
        _ => {
            panic!("un support shader binary");
        }
    }
}

#[derive(Debug)]
pub struct FsTarget {
    states: Vec<ColorTargetState>,
}

impl FsTarget {
    pub fn new_single(state: ColorTargetState) -> Self {
        Self {
            states: vec![state],
        }
    }

    pub fn new(fmt: TextureFormat) -> Self {
        let state = ColorTargetState {
            format: fmt,
            blend: None,
            write_mask: ColorWrites::all(),
        };
        Self::new_single(state)
    }

    pub fn new_blend_alpha_add_mix(fmt: TextureFormat) -> Self {
        let state = ColorTargetState {
            format: fmt,
            blend: Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::OneMinusDstAlpha,
                    dst_factor: BlendFactor::One,
                    operation: BlendOperation::Add,
                },
            }),
            write_mask: ColorWrites::all(),
        };
        Self::new_single(state)
    }
}

use lazy_static::lazy_static;
use spirq::ty::ScalarType;

lazy_static! {
    static ref SIGNED_MAP: HashMap<(u32, u32), VertexFormat> = {
        let mut map = HashMap::new();
        map.insert((1, 2), VertexFormat::Sint8x2);
        map.insert((1, 4), VertexFormat::Sint8x4);
        map.insert((2, 2), VertexFormat::Sint16x2);
        map.insert((2, 4), VertexFormat::Sint16x4);
        map.insert((4, 1), VertexFormat::Sint32);
        map.insert((4, 2), VertexFormat::Sint32x2);
        map.insert((4, 3), VertexFormat::Sint32x3);
        map.insert((4, 4), VertexFormat::Sint32x4);
        map
    };
    static ref UNSIGNED_MAP: HashMap<(u32, u32), VertexFormat> = {
        let mut map = HashMap::new();
        map.insert((1, 2), VertexFormat::Uint8x2);
        map.insert((1, 4), VertexFormat::Uint8x4);
        map.insert((2, 2), VertexFormat::Uint16x2);
        map.insert((2, 4), VertexFormat::Uint16x4);
        map.insert((4, 1), VertexFormat::Uint32);
        map.insert((4, 2), VertexFormat::Uint32x2);
        map.insert((4, 3), VertexFormat::Uint32x3);
        map.insert((4, 4), VertexFormat::Uint32x4);
        map
    };
    static ref FLOAT_MAP: HashMap<(u32, u32), VertexFormat> = {
        let mut map = HashMap::new();
        map.insert((2, 2), VertexFormat::Float16x2);
        map.insert((2, 4), VertexFormat::Float16x4);
        map.insert((4, 1), VertexFormat::Float32);
        map.insert((4, 2), VertexFormat::Float32x2);
        map.insert((4, 3), VertexFormat::Float32x3);
        map.insert((4, 4), VertexFormat::Float32x4);
        map.insert((8, 1), VertexFormat::Float64);
        map.insert((8, 2), VertexFormat::Float64x2);
        map.insert((8, 3), VertexFormat::Float64x3);
        map.insert((8, 4), VertexFormat::Float64x4);
        map
    };
}

fn scalar_to_wgpu_format(stype: &ScalarType, num: u32) -> Option<VertexFormat> {
    match stype {
        ScalarType::Signed(bits) => SIGNED_MAP.get(&(*bits, num)).copied(),
        ScalarType::Unsigned(bits) => UNSIGNED_MAP.get(&(*bits, num)).copied(),
        ScalarType::Float(bits) => FLOAT_MAP.get(&(*bits, num)).copied(),
        _ => None,
    }
}

fn image_to_wgpu_image(arrangement: ImageArrangement) -> Option<TextureViewDimension> {
    match arrangement {
        ImageArrangement::Image1D => Some(TextureViewDimension::D1),
        spirq::ty::ImageArrangement::Image2D => Some(TextureViewDimension::D2),
        spirq::ty::ImageArrangement::Image3D => Some(TextureViewDimension::D3),
        spirq::ty::ImageArrangement::CubeMap => Some(TextureViewDimension::Cube),
        spirq::ty::ImageArrangement::Image2DArray => Some(TextureViewDimension::D2Array),
        spirq::ty::ImageArrangement::CubeMapArray => Some(TextureViewDimension::CubeArray),
        _ => None,
    }
}

impl<'a> PipelineReflector<'a> {
    pub fn new(label: Option<&'static str>, device: &'a Device) -> Self {
        Self {
            label,
            device,
            vs: None,
            fs: None,
            cs: None,
            vertex_attrs: BTreeMap::new(),
            bind_group_layout_entries: BTreeMap::new(),
        }
    }

    fn build_vertex_input(&mut self, entry: &EntryPoint) {
        for input in &entry.vars {
            let format = match input.ty() {
                spirq::ty::Type::Scalar(s) => scalar_to_wgpu_format(s, 1),
                spirq::ty::Type::Vector(t) => scalar_to_wgpu_format(&t.scalar_ty, t.nscalar),
                _ => {
                    continue;
                }
            }
            .unwrap();
            let locator = input.locator();
            if let Locator::Input(t) = locator {
                let position = Position::new(t.comp(), t.loc());
                self.vertex_attrs.insert(position, format);
            }
        }
    }

    fn build_bind_group_layout(&mut self, entry: &EntryPoint, ty: ShaderType) {
        let visibility = match ty {
            ShaderType::Vertex => ShaderStages::VERTEX,
            ShaderType::Fragment => ShaderStages::FRAGMENT,
            ShaderType::Compute => ShaderStages::COMPUTE,
        };

        for desc in &entry.vars {
            if let Variable::Descriptor {
                name,
                desc_bind,
                desc_ty,
                ty,
                nbind,
            } = desc
            {
                let binding = desc_bind.bind();
                let set = desc_bind.set();
                let entry = match desc_ty {
                    spirq::DescriptorType::UniformBuffer() => match ty {
                        spirq::ty::Type::Struct(struct_type) => Some(BindGroupLayoutEntry {
                            binding,
                            visibility: visibility,
                            count: None,
                            ty: BindingType::Buffer {
                                ty: BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                        }),
                        _ => None,
                    },
                    spirq::DescriptorType::SampledImage() => {
                        use spirq::ty::*;
                        match ty {
                            spirq::ty::Type::Image(img) => {
                                let view_dimension = image_to_wgpu_image(img.arng).unwrap();
                                let multisampled = false;
                                let sample_type = match img.unit_fmt {
                                    ImageUnitFormat::Color(t) => {
                                        TextureSampleType::Float { filterable: false }
                                    }
                                    ImageUnitFormat::Sampled => {
                                        TextureSampleType::Float { filterable: false }
                                    }
                                    ImageUnitFormat::Depth => TextureSampleType::Depth,
                                };
                                Some(BindGroupLayoutEntry {
                                    binding,
                                    visibility: visibility,
                                    count: None,
                                    ty: BindingType::Texture {
                                        multisampled,
                                        view_dimension,
                                        sample_type,
                                    },
                                })
                            }
                            _ => None,
                        }
                    }
                    spirq::DescriptorType::Sampler() => Some(BindGroupLayoutEntry {
                        binding,
                        visibility: visibility,
                        count: None,
                        ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
                    }),
                    spirq::DescriptorType::InputAttachment(_) => todo!(),
                    spirq::DescriptorType::AccelStruct() => todo!(),
                    _ => None,
                }
                .unwrap();
                let position = Position::new(set, binding);
                if let Some(item) = self.bind_group_layout_entries.get_mut(&position) {
                    if item.ty != entry.ty {
                        panic!("not eq");
                    } else {
                        item.visibility |= visibility;
                    }
                } else {
                    self.bind_group_layout_entries.insert(position, entry);
                }
            }
        }
    }

    pub fn add_vs(mut self, vs: &ShaderModuleDescriptor) -> Self {
        self.vs = Some(self.device.create_shader_module(vs));
        let vs = make_reflection(vs);
        let entry = ReflectConfig::new()
            .spv(vs)
            .ref_all_rscs(false)
            .reflect()
            .unwrap();
        self.build_vertex_input(&entry[0]);
        self.build_bind_group_layout(&entry[0], ShaderType::Vertex);
        self
    }

    pub fn add_fs(mut self, fs: &ShaderModuleDescriptor, fs_target: FsTarget) -> Self {
        self.fs = Some((self.device.create_shader_module(fs), fs_target));
        let fs = make_reflection(fs);
        let entry = ReflectConfig::new()
            .spv(fs)
            .ref_all_rscs(false)
            .reflect()
            .unwrap();
        self.build_bind_group_layout(&entry[0], ShaderType::Fragment);
        self
    }

    pub fn build(self, primitive: PrimitiveState) -> PipelinePass {
        let label = self.label;
        // build vertex buffer layout firstly
        let mut vertex_buffer_layouts = Vec::new();
        let mut vertex_attrs = Vec::new();
        {
            let mut ranges_size = Vec::new();
            let mut current = (0, 0);
            let mut offset = 0;

            for (pos, format) in self.vertex_attrs {
                if current.0 != pos.set {
                    if current.1 < vertex_attrs.len() {
                        ranges_size.push((current.1..vertex_attrs.len(), offset));
                    }
                    offset = 0;
                    current = (pos.set, vertex_attrs.len());
                }
                vertex_attrs.push(VertexAttribute {
                    format,
                    offset,
                    shader_location: pos.binding,
                });
                offset += format.size();
            }
            if current.1 < vertex_attrs.len() {
                ranges_size.push((current.1..vertex_attrs.len(), offset));
            }
            for (range, size) in ranges_size {
                vertex_buffer_layouts.push(VertexBufferLayout {
                    array_stride: size as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attrs[range],
                });
            }
        }

        // build bind groups secondly
        let mut layouts = Vec::new();
        {
            let mut layout_entries = Vec::new();
            let mut current = 0;
            for (pos, entry) in self.bind_group_layout_entries {
                if current != pos.set {
                    if !layout_entries.is_empty() {
                        let layout =
                            self.device
                                .create_bind_group_layout(&BindGroupLayoutDescriptor {
                                    label,
                                    entries: &layout_entries,
                                });
                        layouts.push(layout);
                        layout_entries.clear();
                    }
                    current = pos.set;
                }
                layout_entries.push(entry);
            }
            if !layout_entries.is_empty() {
                let layout = self
                    .device
                    .create_bind_group_layout(&BindGroupLayoutDescriptor {
                        label,
                        entries: &layout_entries,
                    });
                layouts.push(layout);
            }
        }
        let mut ref_layouts = Vec::new();
        for layout in &layouts {
            ref_layouts.push(layout);
        }
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label,
                bind_group_layouts: &ref_layouts,
                push_constant_ranges: &[],
            });

        let mut pipeline_desc = RenderPipelineDescriptor {
            label,
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &self.vs.unwrap(),
                entry_point: "main",
                buffers: &vertex_buffer_layouts,
            },
            fragment: None,
            primitive,
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
        };

        if let Some(fs) = &self.fs {
            pipeline_desc.fragment = Some(FragmentState {
                module: &fs.0,
                entry_point: "main",
                targets: &fs.1.states,
            })
        }

        log::info!("{:?}", pipeline_desc);

        let pipeline = self.device.create_render_pipeline(&pipeline_desc);

        PipelinePass {
            pipeline,
            bind_group_layouts: layouts,
        }
    }

    pub fn build_default(self) -> PipelinePass {
        self.build(PrimitiveState::default())
    }
}
