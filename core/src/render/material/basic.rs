use std::sync::{Arc, Mutex};


use crate::{
    backends::wgpu_backend::WGPUResource,
    graph::rdg::{
        backend::{GraphCopyEngine, GraphRenderEngine},
        pass::*,
        RenderPassBuilder,
    },
    material::basic::*,
    render::{
        collection::ShaderBindGroupCollection, collector::MeshBufferCollector, pso::{ColorTargetBuilder, RenderDescriptorObject}, tech::ShaderTechCollection
    },
    scene::LayerId,
    util::any_as_u8_slice,
};

use super::{
    take_rs, MaterialRendererFactory, RenderMaterialPsoBuilder, SetupResource
};

struct BasicMaterialHardwareRendererInner {
    shader_bind_group_collection: ShaderBindGroupCollection,
    mesh_buffer_collector: MeshBufferCollector,
    material_shader_collector: Arc<ShaderTechCollection>,
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

        let rs = take_rs::<BasicMaterialFace>(&context)?;
        let c = rs.scene.get_container();

        let layer = rs.layer(self.layer);
        for indirect in &layer.material {
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
            let pso = self.inner.material_shader_collector.get(
                "basic", indirect.material.face().variants(), material.id().id(), "forward");

            self.inner.shader_bind_group_collection.setup(device, material, material.id().id(), pso);
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

            let pso = self.inner.material_shader_collector.get(
                "basic", indirect.material.face().variants(), material.id().id(), "forward");

            pass.set_pipeline(pso.render());
            pass.set_bind_group(0, &layer.main_camera.bind_group, &[0]); // camera bind group

            self.inner.shader_bind_group_collection.bind(&mut pass, material, material.id().id(), &pso);

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
        materials_map: &RenderMaterialPsoBuilder,
        gpu: &WGPUResource,
        g: &mut crate::graph::rdg::RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) {
        for (layer, materials) in &materials_map.map {
            setup_resource
                .shader_tech_collection
                .setup_materials(gpu.device(), materials, "basic", |material, _| {
                    let mut rdo = RenderDescriptorObject::new();
                    rdo = rdo.set_msaa(setup_resource.msaa);

                    if let Some(blend) = material.blend() {
                        rdo = rdo.add_target(
                            ColorTargetBuilder::new(gpu.surface_format())
                                .set_blender(*blend)
                                .build(),
                        );
                    } else {
                        rdo = rdo.add_target(ColorTargetBuilder::new(gpu.surface_format()).build());
                    }
                    let depth_format = wgpu::TextureFormat::Depth32Float;

                    rdo = rdo.set_primitive(|p: &mut _| *p = *material.primitive());
                    rdo = rdo.set_depth(depth_format, |depth: &mut _| {
                        depth.depth_compare = wgpu::CompareFunction::Less;
                        depth.depth_write_enabled = !material.is_transparent();
                    });

                    rdo
                })
                .unwrap();

            let r = Arc::new(Mutex::new(BasicMaterialHardwareRenderer {
                inner: BasicMaterialHardwareRendererInner {
                    mesh_buffer_collector: MeshBufferCollector::new(),
                    shader_bind_group_collection: ShaderBindGroupCollection::new("basic_material_render".into()),
                    material_shader_collector: setup_resource
                        .shader_tech_collection
                        .clone(),
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

