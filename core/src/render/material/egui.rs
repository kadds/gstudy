use std::sync::{Arc, Mutex};

use wgpu::{
    util::{BufferInitDescriptor, DeviceExt, StagingBelt},
    BindGroupDescriptor,
};

use crate::{
    backends::wgpu_backend::{
        FsTarget, GpuInputMainBuffers, GpuMainBuffer, PipelineReflector, WGPUResource,
    },
    ds::PipelineStateObject,
    graph::rdg::{
        pass::RenderPassExecutor, RenderGraphBuilder, RenderPassBuilder, ResourceRegistry,
    },
    material::{egui::EguiMaterialFace, Material, MaterialId},
    ps::{BlendState, CullFace, PrimitiveStateDescriptor},
    render::{DrawCommand, PassIdent},
    scene::Camera,
    types::Vec2f,
    util::any_as_u8_slice,
};

use super::{
    HardwareMaterialShaderCache, MaterialRenderContext, MaterialRenderer, MaterialRendererFactory,
};

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
    // res: Option<(PipelineStateObject, wgpu::Sampler)>,
    // wvp: Option<(wgpu::Buffer, wgpu::BindGroup)>,
    // material_group: HashMap<MaterialId, wgpu::BindGroup>,

    // inner: Option<GpuInputMainBuffers>,
    sampler: wgpu::Sampler,
    commands: Vec<DrawCommand>,
}

impl EguiMaterialHardwareRenderer {
    pub fn name() -> &'static str {
        "egui"
    }
}

impl RenderPassExecutor for EguiMaterialHardwareRenderer {
    fn execute(&self, registry: &ResourceRegistry, pass: &mut wgpu::RenderPass) {
        // let buffer = gpu.new_wvp_buffer::<ScreeSize>(label);

        // let bind = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
        //     label,
        //     layout: &res.pso_ref().pipeline.get_bind_group_layout(0),
        //     entries: &[wgpu::BindGroupEntry {
        //         binding: 0,
        //         resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
        //             buffer: &buffer,
        //             offset: 0,
        //             size: None,
        //         }),
        //     }],
        // });
    }
}

impl MaterialRenderer for EguiMaterialHardwareRenderer {
    // fn render_material<'a, 'b>(
    //     &'a self,
    //     ctx: &'b mut MaterialRenderContext<'b>,
    //     objects: &'b [u64],
    //     material: &'b Material,
    // )  {
    //     todo!()
    // }

    fn render_material<'a, 'b>(
        &'a self,
        ctx: &'b mut super::MaterialRenderContext<'b>,
        objects: &'b [u64],
        material: &'b Material,
        encoder: &mut wgpu::CommandEncoder,
    ) {
    }

    // fn render_material<'a, 'b>(
    //     &mut self,
    //     ctx: &mut super::MaterialRenderContext<'a, 'b>,
    //     objects: &[u64],
    //     material: &Material,
    // ) {
    //     let label = Some("egui");
    //     let mat = material.face_by::<EguiMaterialFace>();
    //     let res = self.res.as_ref().unwrap();
    //     let wvp = self.wvp.as_ref().unwrap();

    //     let pipe_res = ctx.gpu.context().get_resource(res.0.id());

    //     let g = self.material_group.entry(material.id()).or_insert_with(|| {
    //         let group = ctx
    //             .gpu
    //             .device()
    //             .create_bind_group(&wgpu::BindGroupDescriptor {
    //                 label,
    //                 layout: &pipe_res.pso_ref().pipeline.get_bind_group_layout(1),
    //                 entries: &[
    //                     wgpu::BindGroupEntry {
    //                         binding: 0,
    //                         resource: wgpu::BindingResource::TextureView(mat.texture()),
    //                     },
    //                     wgpu::BindGroupEntry {
    //                         binding: 1,
    //                         resource: wgpu::BindingResource::Sampler(&res.1),
    //                     },
    //                 ],
    //             });
    //         group
    //     });

    //     // prepare main buffer
    //     let gpu = ctx.gpu;

    //     let mut total_bytes = (0, 0);

    //     for id in objects {
    //         let object = ctx.scene.get_object(*id).unwrap();
    //         let mesh = object.geometry().mesh();
    //         let vertices = mesh.mixed_mesh();
    //         let indices = mesh.indices();
    //         total_bytes = (
    //             total_bytes.0 + indices.len(),
    //             total_bytes.1 + vertices.len(),
    //         );
    //     }
    //     let inner = self.inner.as_mut().unwrap();

    //     inner.make_sure(gpu, total_bytes.0 as u64, total_bytes.1 as u64);

    //     // copy staging buffer
    //     let mut object_info = Vec::with_capacity(objects.len());

    //     for id in objects {
    //         let object = ctx.scene.get_object(*id).unwrap();
    //         let mesh = object.geometry().mesh();
    //         let vertices = mesh.mixed_mesh();
    //         let indices = mesh.indices();

    //         let result = inner.copy_stage(ctx.encoder.encoder_mut(), gpu, indices, vertices);
    //         object_info.push(result);
    //     }

    //     // draw
    //     let mut pass = ctx.encoder.new_pass();
    //     pass.set_pipeline(&pipe_res.pso_ref().pipeline);

    //     pass.set_bind_group(0, &wvp.1, &[0]);
    //     pass.set_bind_group(1, g, &[]);

    //     for (id, offset) in objects.iter().zip(object_info) {
    //         let object = ctx.scene.get_object(*id).unwrap();
    //         let mesh = object.geometry().mesh();
    //         let index_count = mesh.index_count();

    //         let clip = mesh.clip();
    //         if let Some(clip) = clip {
    //             pass.set_scissor_rect(
    //                 clip.x as u32,
    //                 clip.y as u32,
    //                 (clip.z - clip.x) as u32,
    //                 (clip.w - clip.y) as u32,
    //             );
    //         } else {
    //             let wh = ctx.camera.width_height();
    //             pass.set_scissor_rect(0, 0, wh.x as u32, wh.y as u32);
    //         }

    //         pass.set_index_buffer(
    //             inner.index_buffer_slice(offset.0),
    //             wgpu::IndexFormat::Uint32,
    //         );
    //         pass.set_vertex_buffer(0, inner.vertex_buffer_slice(offset.1));
    //         pass.draw_indexed(0..index_count, 0, 0..1);
    //     }
    // }
}

#[derive(Default)]
pub struct EguiMaterialRendererFactory {}

impl MaterialRendererFactory for EguiMaterialRendererFactory {
    fn setup(
        &self,
        ident: PassIdent,
        material: &[&Material],
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        cache: &mut HardwareMaterialShaderCache,
    ) -> Arc<dyn MaterialRenderer> {
        let egui_texture = g.import_texture();
        let label = Some("egui");

        let r = Arc::new(EguiMaterialHardwareRenderer {
            sampler: gpu.new_sampler(label),
            commands: Vec::new(),
        });

        let mut pass = RenderPassBuilder::new("egui pass");
        pass.read_texture(egui_texture);
        pass.bind_default_render_target();
        pass.async_execute(r.clone());

        g.add_render_pass(pass.build());

        cache.resolve("ui/ui", 0, gpu, |b| {
            let fs_target = FsTarget::new_with_blend(
                gpu.surface_format(),
                &BlendState::default_append_blender(),
            );
            b.add_fs_target(fs_target)
                .with_primitive(PrimitiveStateDescriptor::default().with_cull_face(CullFace::None))
        });
        r
    }

    fn sort_key(&self, material: &Material, gpu: &WGPUResource) -> u64 {
        0
    }
}
