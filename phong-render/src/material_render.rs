use core::{
    backends::wgpu_backend::{ClearValue, ResourceOps},
    graph::rdg::{
        pass::{
            DepthRenderTargetDescriptor, PreferAttachment, RenderPassExecutor,
            RenderTargetDescriptor,
        },
        RenderPassBuilder,
    },
    render::{
        collector::{
            MaterialBufferInstantCollector, MaterialBufferInstantiation, MeshBufferCollector,
        },
        material::{take_rs, MaterialRendererFactory},
        resolve_pipeline, ColorTargetBuilder, PipelinePassResource, RenderDescriptorObject,
        ResolvePipelineConfig,
    },
    scene::Camera,
    types::{Mat3x3f, Mat4x4f, Vec3u, Vec4f},
    util::{any_as_u8_slice, any_as_u8_slice_array},
};
use std::{
    io::Write,
    sync::{Arc, Mutex},
};

use tshader::{LoadTechConfig, ShaderTech};
use wgpu::util::DeviceExt;

use crate::{light::SceneLights, material::PhongMaterialFace};

struct PhongMaterialSceneSharedData {
    lights: Vec<wgpu::Buffer>,
    variants: Vec<&'static str>,
    variants_add: Vec<&'static str>,
}

struct PhongMaterialSharedData {
    material_buffer_collector: MaterialBufferInstantCollector,
    mesh_buffer_collector: MeshBufferCollector,
    scene_shared: Arc<PhongMaterialSceneSharedData>,
}

struct PhongMaterialBaseRenderer {
    shared: Arc<Mutex<PhongMaterialSharedData>>,
}

impl RenderPassExecutor for PhongMaterialBaseRenderer {
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        let rs = take_rs::<PhongMaterialFace>(&context)?;

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
    }

    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();
        let mut shared = self.shared.lock().unwrap();

        for layer in &rs.list {
            let mut pass = engine.begin(layer.layer);

            for indirect in &layer.material {
                let objects = layer.objects(indirect);
                let material = indirect.material.as_ref();

                let (pipeline, material_bind_groups) =
                    shared.material_buffer_collector.get(material);

                pass.set_pipeline(pipeline.render());

                pass.set_bind_group(0, &layer.main_camera.bind_group, &[]); // camera bind group
                pass.set_bind_group(1, material_bind_groups[0].as_ref().unwrap(), &[]); // material bind group
                pass.set_bind_group(2, material_bind_groups[1].as_ref().unwrap(), &[]); // light bind group

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

struct PhongMaterialAddRenderer {
    shared: Arc<Mutex<PhongMaterialSharedData>>,
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
    }

    fn render<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphRenderEngine,
    ) {
        let rs = take_rs::<PhongMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();
        let mut shared = self.shared.lock().unwrap();

        for layer in &rs.list {
            let mut pass = engine.begin(layer.layer);

            for indirect in &layer.material {
                let objects = layer.objects(indirect);
                let material = indirect.material.as_ref();

                let (pipeline, material_bind_groups) =
                    shared.material_buffer_collector.get(material);

                pass.set_pipeline(pipeline.render());

                pass.set_bind_group(0, &layer.main_camera.bind_group, &[]); // camera bind group
                pass.set_bind_group(1, material_bind_groups[0].as_ref().unwrap(), &[]); // material bind group
                pass.set_bind_group(2, material_bind_groups[1].as_ref().unwrap(), &[]); // light bind group

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

pub struct PhongMaterialRendererFactory {}

impl MaterialRendererFactory for PhongMaterialRendererFactory {
    fn setup(
        &self,
        materials: &[std::sync::Arc<core::material::Material>],
        gpu: &core::backends::wgpu_backend::WGPUResource,
        g: &mut core::graph::rdg::RenderGraphBuilder,
        setup_resource: &core::render::material::SetupResource,
    ) {
        let tech = setup_resource
            .shader_loader
            .load_tech(LoadTechConfig {
                name: "phong".into(),
            })
            .unwrap();

        let shadow_tech = setup_resource
            .shader_loader
            .load_tech(LoadTechConfig {
                name: "shadow".into(),
            })
            .unwrap();
        let shadow_template = shadow_tech.register_variant(gpu.device(), &[&[]]).unwrap();

        let mut ins = RenderDescriptorObject::new();

        let depth_format = wgpu::TextureFormat::Depth32Float;

        // ins = ins.set_primitive(|p: &mut _| *p = *material.primitive());
        ins = ins.set_depth(depth_format, |depth: &mut _| {
            depth.depth_compare = wgpu::CompareFunction::Less;
            depth.depth_write_enabled = true;
        });

        let shadow_pipeline = Arc::new(resolve_pipeline(
            &gpu,
            &shadow_template,
            ins,
            &ResolvePipelineConfig {
                constant_stages: vec![wgpu::ShaderStages::VERTEX],
                global_bind_group_layout: Some(&setup_resource.main_camera.bind_group_layout),
            },
        ));

        let mut scene_shared = PhongMaterialSceneSharedData {
            lights: vec![],
            variants: vec![],
            variants_add: vec![],
        };

        let lights = setup_resource.scene.get_resource::<SceneLights>().unwrap();
        let mut light_variants = vec![];

        let extra_lights = lights.extra_lights();
        let has_direct_light = lights.has_direct_light();

        let mut direct_shadow_map = None;

        if has_direct_light {
            light_variants.push("DIRECT_LIGHT");
            let light_uniform = lights.base_uniform();
            let buffer = gpu.new_wvp_buffer_from(Some("Direct light"), &light_uniform);
            scene_shared.lights.push(buffer);
            if lights.direct_light_cast_shadow() {
                let res = g.allocate_texture(
                    "direct shadow map".into(),
                    Vec3u::new(2048, 2048, 1),
                    wgpu::TextureFormat::Depth32Float,
                    wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                    Some(ClearValue::Depth(1f32)),
                    1,
                );
                direct_shadow_map = Some(res);
            }
        }

        scene_shared.variants = light_variants;

        let scene_shared = Arc::new(scene_shared);
        let mut shared = PhongMaterialSharedData {
            material_buffer_collector: MaterialBufferInstantCollector::new(
                PhongMaterialBufferInstantiation {
                    tech: tech.clone(),
                    msaa: setup_resource.msaa,
                    scene_shared: scene_shared.clone(),
                },
            ),
            mesh_buffer_collector: MeshBufferCollector::new(),
            scene_shared: scene_shared.clone(),
        };

        let mut base_pass = RenderPassBuilder::new("phong forward base pass");
        base_pass.default_color_depth_render_target();
        if let Some(shadow_map) = direct_shadow_map {
            base_pass.read_texture(shadow_map);
        }

        let shared = Arc::new(Mutex::new(shared));

        base_pass.async_execute(Arc::new(Mutex::new(PhongMaterialBaseRenderer {
            shared: shared.clone(),
        })));

        g.add_render_pass(base_pass);

        if extra_lights > 0 {
            // add pass
            for i in 1..=extra_lights {
                let mut add_pass = RenderPassBuilder::new(format!("phong forward add pass {}", i));
                add_pass.default_color_depth_render_target();

                add_pass.async_execute(Arc::new(Mutex::new(PhongMaterialAddRenderer {
                    shared: shared.clone(),
                })));

                g.add_render_pass(add_pass);
            }
        }

        // add shadow pass
        if lights.direct_light_cast_shadow() {
            let mut shadow_pass = RenderPassBuilder::new("phong's direct light shadow pass");
            shadow_pass.render_target(RenderTargetDescriptor {
                colors: smallvec::smallvec![],
                depth: Some(DepthRenderTargetDescriptor {
                    prefer_attachment: PreferAttachment::Resource(direct_shadow_map.unwrap()),
                    depth_ops: Some(ResourceOps::load_store()),
                    stencil_ops: None,
                }),
            });
            shadow_pass.async_execute(Arc::new(Mutex::new(ShadowRenderer {
                shared: shared.clone(),
                pipeline: shadow_pipeline,
                cameras: vec![lights.direct_light_camera()],
                cameras_bind_group: vec![],
            })));
            g.add_render_pass(shadow_pass);
        }
    }
}

struct PhongMaterialBufferInstantiation {
    tech: Arc<ShaderTech>,
    msaa: u32,
    scene_shared: Arc<PhongMaterialSceneSharedData>,
}

impl MaterialBufferInstantiation for PhongMaterialBufferInstantiation {
    fn create_pipeline(
        &self,
        material: &core::material::Material,
        global_layout: &wgpu::BindGroupLayout,
        gpu: &core::backends::wgpu_backend::WGPUResource,
    ) -> PipelinePassResource {
        let mut variants = material.face_by::<PhongMaterialFace>().variants.clone();
        let mut variants_add = material.face_by::<PhongMaterialFace>().variants_add.clone();
        variants.extend_from_slice(&self.scene_shared.variants);
        variants_add.extend_from_slice(&self.scene_shared.variants_add);

        let template = self
            .tech
            .register_variant(&gpu.device(), &[&variants, &variants_add])
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
                constant_stages: vec![wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT],
                global_bind_group_layout: Some(global_layout),
            },
        )
    }

    fn create_bind_group(
        &self,
        material: &core::material::Material,
        buffers: &[wgpu::Buffer],
        pipeline: &PipelinePassResource,
        device: &wgpu::Device,
    ) -> Vec<Option<wgpu::BindGroup>> {
        let buffer = &buffers[0];

        let mat = material.face_by::<PhongMaterialFace>();
        let mut entries = vec![];
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer,
                offset: 0,
                size: None,
            }),
        });

        // match &mat.texture {
        //     crate::material::MaterialMap::Texture(texture) => {
        //         let sampler = mat.sampler.as_ref().unwrap();
        //         entries.push(wgpu::BindGroupEntry {
        //             binding: entries.len() as u32,
        //             resource: wgpu::BindingResource::Sampler(sampler.sampler()),
        //         });
        //         entries.push(wgpu::BindGroupEntry {
        //             binding: entries.len() as u32,
        //             resource: wgpu::BindingResource::TextureView(texture.texture_view()),
        //         });
        //     }
        //     _ => (),
        // }

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("basic material"),
            layout: &pipeline.pass[0].get_bind_group_layout(1),
            entries: &entries,
        });

        let mut light_entries = vec![];

        light_entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &self.scene_shared.lights[0],
                offset: 0,
                size: None,
            }),
        });

        let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("basic material"),
            layout: &pipeline.pass[0].get_bind_group_layout(2),
            entries: &light_entries,
        });

        vec![Some(bind_group), Some(light_bind_group)]
    }
}

pub struct ShadowRenderer {
    shared: Arc<Mutex<PhongMaterialSharedData>>,
    pipeline: Arc<PipelinePassResource>,
    cameras: Vec<Arc<Camera>>,
    cameras_bind_group: Vec<(wgpu::Buffer, wgpu::BindGroup)>,
}

impl RenderPassExecutor for ShadowRenderer {
    fn prepare<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        engine: &mut core::graph::rdg::backend::GraphCopyEngine,
    ) -> Option<()> {
        let mut shared = self.shared.lock().unwrap();
        shared.mesh_buffer_collector.recall();
        shared.material_buffer_collector.recall();
        let rs = take_rs::<PhongMaterialFace>(&context)?;
        let c = rs.scene.get_container();

        for layer in &rs.list {
            for indirect in &layer.material {
                let material = indirect.material.as_ref();
                shared
                    .material_buffer_collector
                    .add_pipeline_and_copy_buffer(
                        material,
                        &layer.main_camera.bind_group_layout,
                        &rs.gpu,
                    );
                // create index/vertex buffer
                let objects = layer.objects(indirect);

                for id in objects {
                    shared.mesh_buffer_collector.add(&c, *id, engine.device());
                }
            }
        }
        if self.cameras_bind_group.len() > 0 {
            for (camera, (buffer, _)) in self.cameras.iter().zip(self.cameras_bind_group.iter()) {
                let data = camera.uniform_3d();
                engine.gpu().queue().write_buffer(buffer, 0, &data);
            }
        }
        Some(())
    }

    fn queue<'b>(
        &'b mut self,
        context: core::graph::rdg::pass::RenderPassContext<'b>,
        device: &wgpu::Device,
    ) {
        if self.cameras_bind_group.len() < self.cameras.len() {
            for camera in &self.cameras {
                let data = camera.uniform_3d();
                let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: &data,
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

                    let index_type_u32 = mesh.indices_is_u32().unwrap_or_default();

                    if let Some(index) = &b.index {
                        if index_type_u32 {
                            pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint32);
                        } else {
                            pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint16);
                        }
                    }

                    pass.set_vertex_buffer(0, b.vertex.slice(..));

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
