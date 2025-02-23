use core::{
    backends::wgpu_backend::WGPUResource, graph::rdg::pass::RenderPassExecutor,
    render::{material::take_rs, pso::BindGroupType}, scene::LayerId, types::Mat4x4f, util::any_as_u8_slice_array, wgpu,
};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

use tshader::VariantFlags;

use crate::{light::{SceneLights, TLight}, material::PhongMaterialFace};

use super::{copy_vertex_data, PhongMaterialSharedData};

fn get_object_constant(to_world: &Mat4x4f) -> Vec<u8> {
    let mut constant = vec![];
    let _ = constant.write_all(any_as_u8_slice_array(to_world.as_slice()));
    let to_world3 = to_world.fixed_view::<3, 3>(0, 0);

    if let Some(inv) = to_world3.try_inverse() {
        let p = inv.transpose();
        let p = Mat4x4f::new(
            p.m11, p.m12, p.m13, 0f32, p.m21, p.m22, p.m23, 0f32, p.m31, p.m32, p.m33, 0f32, 0f32,
            0f32, 0f32, 0f32,
        );
        let _ = constant.write_all(any_as_u8_slice_array(p.as_slice()));
    } else {
        log::warn!("inverse object fail");
        let _ = constant.write_all(any_as_u8_slice_array(Mat4x4f::identity().as_slice()));
    }
    constant
}

pub struct PhongMaterialBaseRenderer {
    pub shared: Arc<Mutex<PhongMaterialSharedData>>,
    pub has_shadow_pass: bool,
    pub has_direct_light: bool,
    pub shadow_map_binding: Option<wgpu::BindGroup>,
    pub shadow_map_sampler: Arc<wgpu::Sampler>,
    pub shadow_map_id: Option<u32>,
    pub layer: LayerId,
    pub lights: Arc<SceneLights>,
}

impl RenderPassExecutor for PhongMaterialBaseRenderer {
    #[profiling::function]
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        let mut shared = self.shared.lock().unwrap();
        if !self.has_shadow_pass {
            copy_vertex_data(&mut shared, context, engine.device())?;
        }
        // copy current light uniform
        copy_light_uniform(
            &shared.scene_shared.lights_uniforms[0],
            &shared.scene_shared.lights_buffer[0],
            engine.gpu(),
        );

        Some(())
    }

    #[profiling::function]
    fn queue<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let mut shared = self.shared.lock().unwrap();

        let layer = rs.layer(self.layer);

        for indirect in &layer.material {
            let material = indirect.material.as_ref();
            let pso = shared.material_shader_collector.get(&material, &VariantFlags::default(), 0, "shadow");
            shared
                .shader_bind_group_collection
                .setup(device, material, material.id().id(), pso);
        }

        if self.has_direct_light {
            if let Some(res_id) = &self.shadow_map_id {
                let lights = self.lights;
                shared
                    .shader_bind_group_collection
                    .setup(device, lights.as_ref(), material.id().id(), pso);

                // create shadow map bind_group
                let mut layout = None;

                for indirect in &layer.material {
                    let material = indirect.material.as_ref();
                    let pso = shared.material_shader_collector.get(material, 0);
                    layout = Some(pso.get_bind_group_layout(3));
                    break;
                }
                let shadow_map = context.registry.get(*res_id);

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
    }

    #[profiling::function]
    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();
        let shared = self.shared.lock().unwrap();
        let layer = rs.layer(self.layer);

        let mut pass = engine.begin(layer.layer);

        for indirect in &layer.material {
            let objects = layer.objects(indirect);
            let material = indirect.material.as_ref();

            let pso = shared.material_shader_collector.get(material, 0);

            pass.set_pipeline(pso.render());
            pass.set_bind_group(0, &layer.main_camera.bind_group, &[]); // camera bind group
            shared.shader_bind_group_collection.bind(&mut pass, BindGroupType::Material, material, &pso);
            shared.shader_bind_group_collection.bind(&mut pass, BindGroupType::Light, light, &pso);

            if let Some(s) = self.shadow_map_binding.as_ref() {
                // shared.shader_bind_group_collection.bind(&mut pass, BindGroupType::ShadowUniform, shadow, &pso);
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
                let object_uniform = obj.geometry().transform();

                let constant = get_object_constant(object_uniform.mat());
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

    fn cleanup<'b>(&'b mut self, _context: core::graph::rdg::pass::RenderPassContext<'b>) {}
}

pub struct PhongMaterialAddRenderer {
    pub shared: Arc<Mutex<PhongMaterialSharedData>>,
    pub index: usize,
    pub shadow_map_binding: Option<wgpu::BindGroup>,
    pub shadow_map_sampler: Arc<wgpu::Sampler>,
    pub shadow_map_id: Option<u32>,
    pub has_shadow_pass: bool,
    pub layer: LayerId,
    pub lights: Arc<SceneLights>,
}

impl RenderPassExecutor for PhongMaterialAddRenderer {
    #[profiling::function]
    fn prepare<'b>(
        &'b mut self,
        _context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        let shared = self.shared.lock().unwrap();
        copy_light_uniform(
            &shared.scene_shared.lights_uniforms[self.index + 1],
            &shared.scene_shared.lights_buffer[self.index + 1],
            engine.gpu(),
        );
        Some(())
    }

    #[profiling::function]
    fn queue<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let shared = self.shared.lock().unwrap();
        let layer = rs.layer(self.layer);

        if let Some(res_id) = &self.shadow_map_id {
            let mut layout = None;

            for indirect in &layer.material {
                let material = indirect.material.as_ref();
                let pipeline = shared
                    .material_buffer_collector
                    .get_pass(material, self.index + 1);
                layout = Some(pipeline.0.get_bind_group_layout(3));
                break;
            }
            let shadow_map = context.registry.get(*res_id);
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

    #[profiling::function]
    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();
        let shared = self.shared.lock().unwrap();

        let layer = rs.layer(self.layer);

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

            if let Some(s) = self.shadow_map_binding.as_ref() {
                pass.set_bind_group(3, s, &[]); // shadow bind group
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
                let object_uniform = obj.geometry().transform();
                let constant = get_object_constant(object_uniform.mat());
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

    fn cleanup<'b>(&'b mut self, _context: core::graph::rdg::pass::RenderPassContext<'b>) {}
}
