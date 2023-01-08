use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroU64,
};

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt, StagingBelt},
    BindGroupDescriptor,
};

use crate::{
    backends::wgpu_backend::{
        FsTarget, GpuInputMainBuffers, GpuMainBuffer, PipelineReflector, WGPUResource,
    },
    ds::PipelineStateObject,
    material::{egui::EguiMaterialFace, Material, MaterialId},
    ps::{BlendState, PrimitiveStateDescriptor},
    scene::Camera,
    types::Vec2f,
    util::any_as_u8_slice,
};

use super::{MaterialRenderer, MaterialRendererFactory};

macro_rules! include_egui_shader {
    ($name: tt) => {
        (
            include_bytes!(concat!("../../compile_shaders/ui/", $name, ".vert")),
            include_bytes!(concat!("../../compile_shaders/ui/", $name, ".frag")),
        )
    };
}

struct ScreeSize {
    wh: Vec2f,
}

struct EguiMaterialHardwareRendererInner {
    main_buffer: (GpuMainBuffer, GpuMainBuffer),
    stage_buffer: (StagingBelt, StagingBelt),
}

pub struct EguiMaterialHardwareRenderer {
    res: Option<(PipelineStateObject, wgpu::Sampler)>,
    wvp: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    material_group: HashMap<MaterialId, wgpu::BindGroup>,

    inner: Option<GpuInputMainBuffers>,
}

impl EguiMaterialHardwareRenderer {
    pub fn new() -> Self {
        Self {
            res: None,
            wvp: None,
            material_group: HashMap::new(),
            inner: None,
        }
    }
}

impl MaterialRenderer for EguiMaterialHardwareRenderer {
    fn new_frame(&mut self, gpu: &WGPUResource) {
        self.material_group.clear();
    }

    fn prepare_render(&mut self, gpu: &WGPUResource, camera: &Camera) {
        let label = Some("egui");
        let res = self.res.get_or_insert_with(|| {
            let (vs_source, fs_source) = include_egui_shader!("ui");
            let vs = wgpu::util::make_spirv(vs_source);
            let fs = wgpu::util::make_spirv(fs_source);
            let vs = wgpu::ShaderModuleDescriptor { label, source: vs };
            let fs = wgpu::ShaderModuleDescriptor { label, source: fs };
            let fs_target = FsTarget::new_with_blend(
                camera.render_attachment_format(),
                &BlendState::default_append_blender(),
            );

            let depth = wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            };

            let pass = PipelineReflector::new(label, gpu.device())
                .add_vs(vs)
                .add_fs(fs, fs_target)
                .with_depth(depth)
                .build(
                    PrimitiveStateDescriptor::default().with_cull_face(crate::ps::CullFace::None),
                )
                .unwrap();
            let pso = gpu.context().register_pso(pass);

            (pso, gpu.new_sampler(label))
        });

        let res = gpu.context().get_resource(res.0.id());

        let wvp = self.wvp.get_or_insert_with(|| {
            let buffer = gpu.new_wvp_buffer::<ScreeSize>(label);

            let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout: &res.pso_ref().pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

            (buffer, bind)
        });

        let wvp_data = ScreeSize {
            wh: camera.width_height(),
        };
        gpu.queue()
            .write_buffer(&wvp.0, 0, any_as_u8_slice(&wvp_data));

        let inner = self
            .inner
            .get_or_insert_with(|| GpuInputMainBuffers::new(gpu, label));
        inner.finish();
        inner.recall();
    }

    fn render_material<'a, 'b>(
        &mut self,
        ctx: &mut super::MaterialRenderContext<'a, 'b>,
        objects: &[u64],
        material: &Material,
    ) {
        let label = Some("egui");
        let mat = material.face_by::<EguiMaterialFace>();
        let res = self.res.as_ref().unwrap();
        let wvp = self.wvp.as_ref().unwrap();

        let pipe_res = ctx.gpu.context().get_resource(res.0.id());

        let g = self.material_group.entry(material.id()).or_insert_with(|| {
            let group = ctx
                .gpu
                .device()
                .create_bind_group(&wgpu::BindGroupDescriptor {
                    label,
                    layout: &pipe_res.pso_ref().pipeline.get_bind_group_layout(1),
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(mat.texture()),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&res.1),
                        },
                    ],
                });
            group
        });

        // prepare main buffer
        let gpu = ctx.gpu;

        let mut total_bytes = (0, 0);

        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let vertices = mesh.mixed_mesh();
            let indices = mesh.indices();
            total_bytes = (
                total_bytes.0 + indices.len(),
                total_bytes.1 + vertices.len(),
            );
        }
        let inner = self.inner.as_mut().unwrap();

        inner.make_sure(gpu, total_bytes.0 as u64, total_bytes.1 as u64);

        // copy staging buffer
        let mut object_info = Vec::with_capacity(objects.len());

        for id in objects {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let vertices = mesh.mixed_mesh();
            let indices = mesh.indices();

            let result = inner.copy_stage(ctx.encoder.encoder_mut(), gpu, indices, vertices);
            object_info.push(result);
        }

        // draw
        let mut pass = ctx.encoder.new_pass();
        pass.set_pipeline(&pipe_res.pso_ref().pipeline);

        pass.set_bind_group(0, &wvp.1, &[0]);
        pass.set_bind_group(1, g, &[]);

        for (id, offset) in objects.iter().zip(object_info) {
            let object = ctx.scene.get_object(*id).unwrap();
            let mesh = object.geometry().mesh();
            let index_count = mesh.index_count();

            let clip = mesh.clip();
            if let Some(clip) = clip {
                pass.set_scissor_rect(
                    clip.x as u32,
                    clip.y as u32,
                    (clip.z - clip.x) as u32,
                    (clip.w - clip.y) as u32,
                );
            } else {
                let wh = ctx.camera.width_height();
                pass.set_scissor_rect(0, 0, wh.x as u32, wh.y as u32);
            }

            pass.set_index_buffer(
                inner.index_buffer_slice(offset.0),
                wgpu::IndexFormat::Uint32,
            );
            pass.set_vertex_buffer(0, inner.vertex_buffer_slice(offset.1));
            pass.draw_indexed(0..index_count, 0, 0..1);
        }
    }

    fn sort_key(&mut self, material: &Material, gpu: &WGPUResource) -> u64 {
        0
    }
}

#[derive(Default)]
pub struct EguiMaterialRendererFactory {}

impl MaterialRendererFactory for EguiMaterialRendererFactory {
    fn new(&self) -> Box<dyn MaterialRenderer> {
        Box::new(EguiMaterialHardwareRenderer::new())
    }
}
