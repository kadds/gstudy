use core::{
    graph::rdg::pass::RenderPassExecutor,
    render::{material::take_rs, PipelinePassResource},
    types::Vec2f,
    util::any_as_u8_slice,
    wgpu::{self, util::DeviceExt},
};
use std::sync::{Arc, Mutex};

use crate::{
    light::{Light, TLight},
    material::PhongMaterialFace,
};

use super::{copy_vertex_data, PhongMaterialSharedData};

pub struct ShadowRenderer {
    pub shared: Arc<Mutex<PhongMaterialSharedData>>,
    pub pipeline: Arc<PipelinePassResource>,
    pub light: Arc<Light>,
    pub cameras_bind_group: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
    pub size: Vec2f,
}

impl RenderPassExecutor for ShadowRenderer {
    #[profiling::function]
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        let mut shared = self.shared.lock().unwrap();
        copy_vertex_data(&mut shared, context, engine.device())?;

        if !self.cameras_bind_group.is_empty() {
            let data = self.light.shadow_uniform();
            engine
                .gpu()
                .queue()
                .write_buffer(&self.cameras_bind_group[0].0, 0, &data);
        }
        Some(())
    }

    #[profiling::function]
    fn queue<'b>(
        &'b mut self,
        _context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
        if self.cameras_bind_group.is_empty() {
            let uniform = self.light.shadow_uniform();

            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &uniform,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.pipeline.pass[0].get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &buffer,
                        offset: 0,
                        size: None,
                    }),
                }],
            });
            self.cameras_bind_group.push((buffer, bind_group));
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

        for layer in &rs.list {
            let mut pass = engine.begin(layer.layer);
            pass.set_viewport(0f32, 0f32, self.size.x, self.size.y, 0.01f32, 1f32);

            for indirect in &layer.material {
                let objects = layer.objects(indirect);

                pass.set_pipeline(self.pipeline.pass[0].render());
                pass.set_bind_group(0, &self.cameras_bind_group[0].1, &[]); // camera bind group

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
                        wgpu::ShaderStages::VERTEX,
                        0,
                        any_as_u8_slice(object_uniform.mat()),
                    );

                    let b = shared.mesh_buffer_collector.get(&c, *id).unwrap();

                    b.draw_no_properties(&mesh, &mut pass);

                    pass.pop_debug_group();
                }
            }
        }
    }

    #[profiling::function]
    fn cleanup<'b>(&'b mut self, _context: core::graph::rdg::pass::RenderPassContext<'b>) {}
}
