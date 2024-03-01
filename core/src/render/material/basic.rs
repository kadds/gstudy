use std::sync::{Arc, Mutex};

use tshader::{LoadTechConfig, ShaderTech};

use crate::{
    backends::wgpu_backend::WGPUResource,
    graph::rdg::{
        backend::{GraphCopyEngine, GraphRenderEngine},
        pass::*,
        RenderPassBuilder,
    },
    material::{basic::*, Material},
    render::{
        collector::{
            MaterialBufferInstantCollector, MaterialBufferInstantiation, MeshBufferCollector,
        },
        resolve_pipeline, ColorTargetBuilder, PipelinePassResource, RenderDescriptorObject,
        ResolvePipelineConfig,
    },
    scene::LayerId,
    util::any_as_u8_slice,
};

use super::{take_rs, MaterialRendererFactory, RenderMaterialBuilderMap, SetupResource};

struct BasicMaterialHardwareRendererInner {
    material_buffer_collector: MaterialBufferInstantCollector,
    mesh_buffer_collector: MeshBufferCollector,
}

pub struct BasicMaterialHardwareRenderer {
    inner: BasicMaterialHardwareRendererInner,
    layer: LayerId,
}

impl RenderPassExecutor for BasicMaterialHardwareRenderer {
    #[profiling::function]
    fn prepare<'a>(
        &'a mut self,
        context: RenderPassContext<'a>,
        engine: &mut GraphCopyEngine,
    ) -> Option<()> {
        self.inner.mesh_buffer_collector.recall();
        self.inner.material_buffer_collector.recall();

        let rs = take_rs::<BasicMaterialFace>(&context)?;
        let c = rs.scene.get_container();

        let layer = rs.layer(self.layer);
        for indirect in &layer.material {
            let material = indirect.material.as_ref();
            self.inner
                .material_buffer_collector
                .add_pipeline_and_copy_buffer(
                    material,
                    &layer.main_camera.bind_group_layout,
                    &rs.gpu,
                );

            // create index/vertex buffer
            let objects = layer.objects(indirect);

            for id in objects {
                self.inner
                    .mesh_buffer_collector
                    .add(&c, *id, engine.device());
            }
        }

        Some(())
    }

    #[profiling::function]
    fn queue<'b>(&'b mut self, context: RenderPassContext<'b>, device: &wgpu::Device) {
        let rs = take_rs::<BasicMaterialFace>(&context).unwrap();
        let layer = rs.layer(self.layer);

        for indirect in &layer.material {
            let material = indirect.material.as_ref();
            self.inner
                .material_buffer_collector
                .add_bind_group(material, device);
        }
    }

    #[profiling::function]
    fn render<'a>(&'a mut self, context: RenderPassContext<'a>, engine: &mut GraphRenderEngine) {
        let rs = take_rs::<BasicMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();

        let layer = rs.layer(self.layer);
        let mut pass = engine.begin(layer.layer);

        for indirect in &layer.material {
            let objects = layer.objects(indirect);
            let material = indirect.material.as_ref();

            let (pipeline, material_bind_groups) =
                self.inner.material_buffer_collector.get(material);

            pass.set_pipeline(pipeline.render());
            pass.set_bind_group(0, &layer.main_camera.bind_group, &[]); // camera bind group
            if let Some(b) = &material_bind_groups[0] {
                pass.set_bind_group(1, b, &[]); // material bind group
            }

            // object bind_group
            for id in objects {
                let obj = match c.get(id) {
                    Some(v) => v,
                    None => continue,
                };
                let obj = obj.o();
                pass.push_debug_group(&format!("object {}", obj.name()));
                let mesh = obj.geometry().mesh();
                let b = self.inner.mesh_buffer_collector.get(&c, *id).unwrap();

                if b.instance_data.is_none() {
                    let object_uniform = obj.geometry().transform();
                    pass.set_push_constants(
                        wgpu::ShaderStages::VERTEX,
                        0,
                        any_as_u8_slice(object_uniform.mat()),
                    );
                }

                b.draw(&mesh, &mut pass);

                pass.pop_debug_group();
            }
        }
    }

    #[profiling::function]
    fn cleanup<'b>(&'b mut self, context: RenderPassContext<'b>) {
        let _rs = take_rs::<BasicMaterialFace>(&context).unwrap();
        self.inner.mesh_buffer_collector.finish();
    }
}

#[derive(Default)]
pub struct BasicMaterialRendererFactory {}

impl MaterialRendererFactory for BasicMaterialRendererFactory {
    fn setup(
        &self,
        materials_map: &RenderMaterialBuilderMap,
        _gpu: &WGPUResource,
        g: &mut crate::graph::rdg::RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) {
        let tech = setup_resource
            .shader_loader
            .load_tech(LoadTechConfig {
                name: "basic_forward".into(),
            })
            .unwrap();

        for (layer, _) in materials_map {
            let r = Arc::new(Mutex::new(BasicMaterialHardwareRenderer {
                inner: BasicMaterialHardwareRendererInner {
                    material_buffer_collector: MaterialBufferInstantCollector::new(
                        BasicMaterialBufferInstantiation {
                            tech: tech.clone(),
                            msaa: setup_resource.msaa,
                        },
                    ),
                    mesh_buffer_collector: MeshBufferCollector::new(),
                },
                layer: *layer,
            }));

            let mut pass = RenderPassBuilder::new(format!("basic render pass layer {}", layer));
            pass.default_color_depth_render_target();
            pass.async_execute(r.clone());
            pass.add_constraint(PassConstraint::Last);

            g.add_render_pass(pass);
        }
    }
}

struct BasicMaterialBufferInstantiation {
    tech: Arc<ShaderTech>,
    msaa: u32,
}

impl MaterialBufferInstantiation for BasicMaterialBufferInstantiation {
    #[profiling::function]
    fn create_pipeline(
        &self,
        material: &Material,
        global_layout: &wgpu::BindGroupLayout,
        gpu: &WGPUResource,
    ) -> PipelinePassResource {
        let variants = &material.face_by::<BasicMaterialFace>().variants;
        let template = self
            .tech
            .register_variant(&gpu.device(), &[variants])
            .unwrap();

        let mut ins = RenderDescriptorObject::new();
        ins = ins.set_msaa(self.msaa);

        if let Some(blend) = material.blend() {
            ins = ins.add_target(
                ColorTargetBuilder::new(gpu.surface_format())
                    .set_blender(*blend)
                    .build(),
            );
        } else {
            ins = ins.add_target(ColorTargetBuilder::new(gpu.surface_format()).build());
        }
        let depth_format = wgpu::TextureFormat::Depth32Float;

        ins = ins.set_primitive(|p: &mut _| *p = *material.primitive());
        ins = ins.set_depth(depth_format, |depth: &mut _| {
            depth.depth_compare = wgpu::CompareFunction::Less;
            depth.depth_write_enabled = !material.is_transparent();
        });

        resolve_pipeline(
            &gpu,
            &template,
            ins,
            &ResolvePipelineConfig {
                constant_stages: vec![wgpu::ShaderStages::VERTEX],
                global_bind_group_layout: Some(global_layout),
            },
        )
    }

    #[profiling::function]
    fn create_bind_group(
        &self,
        material: &Material,
        buffers: &[wgpu::Buffer],
        pipeline: &PipelinePassResource,
        device: &wgpu::Device,
    ) -> Vec<Option<wgpu::BindGroup>> {
        let buffer = &buffers[0];

        let mat = material.face_by::<BasicMaterialFace>();
        let mut entries = vec![];
        if buffer.size() != 0 {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer,
                    offset: 0,
                    size: None,
                }),
            });
        }

        if let Some(texture) = mat.texture.texture_ref() {
            let sampler = mat.sampler.as_ref().unwrap();
            entries.push(wgpu::BindGroupEntry {
                binding: entries.len() as u32,
                resource: wgpu::BindingResource::Sampler(sampler.sampler()),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: entries.len() as u32,
                resource: wgpu::BindingResource::TextureView(texture.texture_view()),
            });
        }

        if entries.len() == 0 {
            return vec![None];
        }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("basic material"),
            layout: &pipeline.pass[0].get_bind_group_layout(1),
            entries: &entries,
        });

        vec![Some(bind_group)]
    }
}
