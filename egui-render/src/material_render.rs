use std::{
    mem::size_of,
    ops::Range,
    sync::{Arc, Mutex},
};

use core::{
    backends::wgpu_backend::{ClearValue, GpuInputMainBuffers, ResourceOps, WGPUResource},
    context::ResourceRef,
    graph::rdg::{backend::GraphCopyEngine, pass::*, RenderGraphBuilder, RenderPassBuilder},
    material::{bind::{BindingResourceProvider, ShaderBindingResource}, Material},
    render::{
        collection::ShaderBindGroupCollection,
        material::{take_rs, MaterialRendererFactory, RenderMaterialPsoBuilder, SetupResource},
        pso::{BindGroupType, ColorTargetBuilder, RenderDescriptorObject},
        tech::ShaderTechCollection,
    },
    scene::LayerId,
    types::Rectu,
    wgpu,
};

use crate::material::EguiMaterialFace;

struct EguiMaterialHardwareRendererInner {
    main_buffers: GpuInputMainBuffers,
    sampler: ResourceRef,

    shader_bind_group_collection: ShaderBindGroupCollection,
    material_shader_collector: Arc<ShaderTechCollection>,

    draw_index_buffer: Vec<(Range<u32>, i32, Option<Rectu>)>,
}

struct EguiMaterialShaderResourceProvider<'a> {
    mat: &'a Material,
    sampler: ResourceRef,
}

impl<'a> BindingResourceProvider for EguiMaterialShaderResourceProvider<'a> {
    fn query_resource(&self,key: &str) -> ShaderBindingResource {
        if key == "texture_sampler" {
            return self.sampler.clone().into();
        }
        self.mat.query_resource(key)
    }

    fn bind_group(&self) -> BindGroupType {
        self.mat.bind_group()
    }
}

pub struct EguiMaterialHardwareRenderer {
    inner: EguiMaterialHardwareRendererInner,
    layer: LayerId,
}

impl EguiMaterialHardwareRenderer {}

impl RenderPassExecutor for EguiMaterialHardwareRenderer {
    #[profiling::function]
    fn prepare<'a>(
        &'a mut self,
        context: RenderPassContext<'a>,
        engine: &mut GraphCopyEngine,
    ) -> Option<()> {
        let inner = &mut self.inner;
        inner.main_buffers.recall();
        let rs = take_rs::<EguiMaterialFace>(&context)?;
        let c = rs.scene.get_container();

        // copy vertices and indices
        let gpu_ref = engine.gpu_ref();

        let layer = rs.layer(self.layer);

        for indirect in &layer.material {
            let objects = layer.objects(indirect);

            for id in objects {
                let obj = match c.get(id) {
                    Some(v) => v,
                    None => continue,
                };
                let obj = obj.o();
                let mesh = obj.geometry().mesh();
                let indices = mesh.indices_view().unwrap();
                let vertices = mesh.properties_view();

                inner
                    .main_buffers
                    .prepare(&gpu_ref, indices.len() as u64, vertices.len() as u64);

                let (is, vs) =
                    inner
                        .main_buffers
                        .copy_stage(engine.encoder(), &gpu_ref, indices, vertices);

                let index_size = size_of::<u32>() as u64;
                let vs_size = mesh.row_strip_size() as u64;
                let vs = (vs.start / vs_size) as i32;

                inner.draw_index_buffer.push((
                    (is.start / index_size) as u32..(is.end / index_size) as u32,
                    vs,
                    mesh.clip(),
                ));
            }
        }

        self.inner.main_buffers.finish();

        Some(())
    }

    #[profiling::function]
    fn queue<'b>(&'b mut self, context: RenderPassContext<'b>, device: &wgpu::Device) {
        let inner = &mut self.inner;

        let rs = take_rs::<EguiMaterialFace>(&context).unwrap();
        let layer = rs.layer(self.layer);
        for indirect in &layer.material {
            let pso = inner
                .material_shader_collector
                .get("egui", indirect.material.face().variants(), indirect.material.id().id(), "egui");

            let rp = EguiMaterialShaderResourceProvider {
                mat: &indirect.material,
                sampler: inner.sampler.clone(),
            };
            inner.shader_bind_group_collection.setup(
                device,
                &rp,
                indirect.material.id().id(),
                pso,
            );
        }
    }

    #[profiling::function]
    fn render<'b>(
        &'b mut self,
        context: RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let inner = &mut self.inner;

        let rs = take_rs::<EguiMaterialFace>(&context).unwrap();
        let layer = rs.layer(self.layer);
        let mut pass = engine.begin(layer.layer);

        for indirect in &layer.material {
            let pso = inner
                .material_shader_collector
                .get("egui", indirect.material.face().variants(), indirect.material.id().id(), "egui");

            pass.set_pipeline(pso.render());
            pass.set_bind_group(0, &layer.main_camera.bind_group, &[0]);
            pass.set_index_buffer(
                inner.main_buffers.index().buffer().slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.set_vertex_buffer(0, inner.main_buffers.vertex().buffer().slice(..));

            let rp = EguiMaterialShaderResourceProvider {
                mat: &indirect.material,
                sampler: inner.sampler.clone(),
            };

            inner.shader_bind_group_collection.bind(
                &mut pass,
                &rp,
                indirect.material.id().id(),
                &pso,
            );

            for (indices, vertices, rect) in &inner.draw_index_buffer {
                if let Some(r) = rect {
                    pass.set_scissor_rect(r.x, r.y, r.z, r.w);
                }
                pass.draw_indexed(indices.clone(), *vertices, 0..1);
            }
        }
    }

    #[profiling::function]
    fn cleanup<'b>(&'b mut self, _context: RenderPassContext<'b>) {
        self.inner.draw_index_buffer.clear();
    }
}

#[derive(Default)]
pub struct EguiMaterialRendererFactory {}

impl MaterialRendererFactory for EguiMaterialRendererFactory {
    fn setup(
        &self,
        materials_map: &RenderMaterialPsoBuilder,
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) {
        let ctx = gpu.context();
        let label = Some("egui");

        let sampler = gpu.device().create_sampler(&wgpu::SamplerDescriptor {
            label,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0f32,
            lod_max_clamp: f32::MAX,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });
        let sampler = ctx.register_sampler(sampler);

        let depth_format = wgpu::TextureFormat::Depth32Float;

        for (layer, materials) in &materials_map.map {
            setup_resource
                .shader_tech_collection
                .setup_materials(gpu.device(), materials, "egui", |material, _| {
                    let mut rdo = RenderDescriptorObject::new();
                    rdo = rdo
                        .set_depth(depth_format, |depth: &mut _| {
                            depth.depth_compare = wgpu::CompareFunction::LessEqual;
                        })
                        .vertex_no_split()
                        .set_primitive(|primitive: &mut _| {
                            primitive.cull_mode = None;
                        })
                        .set_msaa(setup_resource.msaa)
                        .add_target(
                            ColorTargetBuilder::new(gpu.surface_format())
                                .set_append_blender()
                                .build(),
                        );

                    rdo
                })
                .unwrap();

            let r = Arc::new(Mutex::new(EguiMaterialHardwareRenderer {
                inner: EguiMaterialHardwareRendererInner {
                    main_buffers: GpuInputMainBuffers::new(gpu, label),
                    sampler: sampler.clone(),
                    draw_index_buffer: vec![],
                    material_shader_collector: setup_resource
                        .shader_tech_collection
                        .clone(),
                    shader_bind_group_collection: ShaderBindGroupCollection::new(
                        "egui_material_render".into(),
                    ),
                },
                layer: *layer,
            }));

            let mut pass = RenderPassBuilder::new(format!("egui pass layer {}", layer));
            pass.render_target(RenderTargetDescriptor {
                colors: smallvec::smallvec![ColorRenderTargetDescriptor {
                    prefer_attachment: PreferAttachment::Default,
                    resolve_attachment: PreferAttachment::Default,
                    ops: ResourceOps {
                        load: None,
                        store: true,
                    },
                }],
                depth: Some(DepthRenderTargetDescriptor {
                    prefer_attachment: PreferAttachment::Default,
                    depth_ops: Some(ResourceOps {
                        load: Some(ClearValue::Depth(1.0f32)),
                        store: true,
                    }),
                    stencil_ops: None,
                }),
            });

            pass.add_constraint(PassConstraint::Last);

            pass.async_execute(r.clone());

            g.add_render_pass(pass);
        }
    }
}
