use core::{
    graph::rdg::pass::RenderPassExecutor,
    render::material::take_rs,
    types::Mat4x4f,
    util::{any_as_u8_slice, any_as_u8_slice_array},
};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

use crate::material::PhongMaterialFace;

use super::{copy_vertex_data, PhongMaterialSharedData, ShadowMap};

pub struct PhongMaterialBaseRenderer {
    pub shared: Arc<Mutex<PhongMaterialSharedData>>,
    pub has_shadow_pass: bool,
    pub shadow_map_binding: Option<wgpu::BindGroup>,
    pub shadow_map_sampler: Arc<wgpu::Sampler>,
    pub shadow_map_id: ShadowMap,
}
impl RenderPassExecutor for PhongMaterialBaseRenderer {
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        let mut shared = self.shared.lock().unwrap();
        if !self.has_shadow_pass {
            copy_vertex_data(&mut shared, context, engine.device())?;
        }

        Some(())
    }

    fn queue<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let mut shared = self.shared.lock().unwrap();

        for layer in &rs.list {
            for indirect in &layer.material {
                let material = indirect.material.as_ref();
                shared
                    .material_buffer_collector
                    .add_bind_group(material, device);
            }
        }

        // create shadow map bind_group
        if self.has_shadow_pass {
            let mut layout = None;

            for layer in &rs.list {
                for indirect in &layer.material {
                    let material = indirect.material.as_ref();
                    let pipeline = shared.material_buffer_collector.get(&material);
                    layout = Some(pipeline.0.get_bind_group_layout(3));
                }
            }
            let shadow_map = match &self.shadow_map_id {
                ShadowMap::BuiltIn(res) => res.clone(),
                ShadowMap::PreFrame(id) => context.registry.get(*id),
            };
            let mut entries = vec![];
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(self.shadow_map_sampler.as_ref()),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(shadow_map.texture_view()),
            });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("phong shadow map bind group"),
                entries: &entries,
                layout: layout.as_ref().unwrap(),
            });
            self.shadow_map_binding = Some(bind_group)
        }
    }

    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();
        let shared = self.shared.lock().unwrap();

        for layer in &rs.list {
            let mut pass = engine.begin(layer.layer);

            for indirect in &layer.material {
                let objects = layer.objects(indirect);
                let material = indirect.material.as_ref();

                let (pipeline, material_bind_groups) =
                    shared.material_buffer_collector.get(material);

                pass.set_pipeline(pipeline.render());

                pass.set_bind_group(0, &layer.main_camera.bind_group, &[]); // camera bind group
                pass.set_bind_group(2, material_bind_groups[0].as_ref().unwrap(), &[]); // light bind group
                pass.set_bind_group(1, material_bind_groups[1].as_ref().unwrap(), &[]); // material bind group
                pass.set_bind_group(3, self.shadow_map_binding.as_ref().unwrap(), &[]); // light bind group

                // object bind_group
                for id in objects {
                    let obj = match c.get(id) {
                        Some(v) => v,
                        None => continue,
                    };
                    let obj = obj.o();
                    pass.push_debug_group(&format!("object {}", obj.name()));
                    let mesh = obj.geometry().mesh();
                    let object_uniform = obj.geometry().transform();

                    let to_world = object_uniform.mat();

                    let mut constant = vec![];
                    constant.write_all(any_as_u8_slice_array(to_world.as_slice()));
                    let to_world3 = to_world.fixed_view::<3, 3>(0, 0);

                    if let Some(inv) = to_world3.try_inverse() {
                        let p = inv.transpose();
                        let p = Mat4x4f::new(
                            p.m11, p.m12, p.m13, 0f32, p.m21, p.m22, p.m23, 0f32, p.m31, p.m32,
                            p.m33, 0f32, 0f32, 0f32, 0f32, 0f32,
                        );
                        constant.write_all(any_as_u8_slice_array(p.as_slice()));
                    } else {
                        log::warn!("inverse object {} fail", obj.name());
                        constant.write_all(any_as_u8_slice_array(Mat4x4f::identity().as_slice()));
                    }
                    // constant.write_all(any_as_u8_slice_array(Vec4f::zeros().as_slice()));

                    pass.set_push_constants(
                        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        0,
                        &constant,
                    );

                    let b = shared.mesh_buffer_collector.get(&c, *id).unwrap();
                    b.draw(&mesh, &mut pass);

                    pass.pop_debug_group();
                }
            }
        }
    }

    fn cleanup<'b>(&'b mut self, context: core::graph::rdg::pass::RenderPassContext<'b>) {}
}

pub struct PhongMaterialAddRenderer {
    pub shared: Arc<Mutex<PhongMaterialSharedData>>,
    pub index: usize,
    pub shadow_map_binding: Option<wgpu::BindGroup>,
    pub shadow_map_sampler: Arc<wgpu::Sampler>,
    pub shadow_map_id: ShadowMap,
}

impl RenderPassExecutor for PhongMaterialAddRenderer {
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        Some(())
    }

    fn queue<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let shared = self.shared.lock().unwrap();
        let mut layout = None;

        for layer in &rs.list {
            for indirect in &layer.material {
                let material = indirect.material.as_ref();
                let pipeline = shared.material_buffer_collector.get(&material);
                layout = Some(pipeline.0.get_bind_group_layout(3));
            }
        }
        let shadow_map = match &self.shadow_map_id {
            ShadowMap::BuiltIn(res) => res.clone(),
            ShadowMap::PreFrame(id) => context.registry.get(*id),
        };
        let mut entries = vec![];
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Sampler(self.shadow_map_sampler.as_ref()),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: wgpu::BindingResource::TextureView(shadow_map.texture_view()),
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("basic shadow map bind group"),
            entries: &entries,
            layout: layout.as_ref().unwrap(),
        });
        self.shadow_map_binding = Some(bind_group)
    }

    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();
        let shared = self.shared.lock().unwrap();

        for layer in &rs.list {
            let mut pass = engine.begin(layer.layer);

            for indirect in &layer.material {
                let objects = layer.objects(indirect);
                let material = indirect.material.as_ref();

                let (pipeline, material_bind_groups) = shared
                    .material_buffer_collector
                    .get_pass(material, self.index + 1);

                pass.set_pipeline(pipeline.render());

                pass.set_bind_group(0, &layer.main_camera.bind_group, &[]); // camera bind group
                pass.set_bind_group(2, material_bind_groups[0].as_ref().unwrap(), &[]); // light bind group
                pass.set_bind_group(
                    1,
                    material_bind_groups[self.index + 2].as_ref().unwrap(),
                    &[],
                ); // material bind group
                pass.set_bind_group(3, self.shadow_map_binding.as_ref().unwrap(), &[]); // light bind group

                // object bind_group
                for id in objects {
                    let obj = match c.get(id) {
                        Some(v) => v,
                        None => continue,
                    };
                    let obj = obj.o();
                    pass.push_debug_group(&format!("object {}", obj.name()));
                    let mesh = obj.geometry().mesh();
                    let object_uniform = obj.geometry().transform();
                    pass.set_push_constants(
                        wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        0,
                        any_as_u8_slice(object_uniform.mat()),
                    );

                    let b = shared.mesh_buffer_collector.get(&c, *id).unwrap();

                    let index_type_u32 = mesh.indices_is_u32().unwrap_or_default();

                    if let Some(index) = &b.index {
                        if index_type_u32 {
                            pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint32);
                        } else {
                            pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint16);
                        }
                    }

                    pass.set_vertex_buffer(0, b.vertex.slice(..));
                    if let Some(properties) = &b.vertex_properties {
                        pass.set_vertex_buffer(1, properties.slice(..));
                    }

                    // index
                    if b.index.is_some() {
                        pass.draw_indexed(0..mesh.index_count().unwrap(), 0, 0..1);
                    } else {
                        pass.draw(0..mesh.vertex_count() as u32, 0..1);
                    }
                    pass.pop_debug_group();
                }
            }
        }
    }

    fn cleanup<'b>(&'b mut self, context: core::graph::rdg::pass::RenderPassContext<'b>) {}
}
