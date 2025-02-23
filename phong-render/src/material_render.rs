use core::{
    backends::wgpu_backend::{ClearValue, ResourceOps},
    graph::rdg::{
        pass::{DepthRenderTargetDescriptor, PreferAttachment, RenderTargetDescriptor},
        RenderPassBuilder,
    },
    render::{
        collection::ShaderBindGroupCollection,
        collector::MeshBufferCollector,
        material::{take_rs, MaterialRendererFactory, RenderMaterialPsoBuilder},
        pso::{PipelineStateObject, RenderDescriptorObject},
        tech::ShaderTechCollection,
    },
    types::Vec3u,
    wgpu,
};
use std::sync::{Arc, Mutex};

use tshader::{ShaderTech, VariantFlags};

mod base;
mod shadow;

use crate::{
    light::{BaseLightUniform, Light, SceneLights, TLight},
    material::PhongMaterialFace,
};

use self::shadow::ShadowRenderer;

struct PhongMaterialSceneSharedData {
    variants_base: Vec<&'static str>,
    variants_add: Vec<Vec<&'static str>>,
}

pub struct PhongMaterialSharedData {
    mesh_buffer_collector: MeshBufferCollector,
    shader_bind_group_collection: ShaderBindGroupCollection,
    material_shader_collector: Arc<ShaderTechCollection>,

    scene_shared: Arc<PhongMaterialSceneSharedData>,
}

fn copy_vertex_data(
    shared: &mut PhongMaterialSharedData,
    context: core::graph::rdg::pass::RenderPassContext<'_>,
    device: &wgpu::Device,
) -> Option<()> {
    shared.mesh_buffer_collector.recall();

    let rs = take_rs::<PhongMaterialFace>(&context)?;
    let c = rs.scene.get_container();

    for layer in &rs.list {
        for indirect in &layer.material {
            // create index/vertex buffer
            let objects = layer.objects(indirect);

            for id in objects {
                shared.mesh_buffer_collector.add(&c, *id, device);
            }
        }
    }
    Some(())
}

pub struct PhongMaterialRendererFactory {}

impl PhongMaterialRendererFactory {
    fn add_shadow_pass_for_light(
        &self,
        t: Arc<Light>,
        shared: Arc<Mutex<PhongMaterialSharedData>>,
        g: &mut core::graph::rdg::RenderGraphBuilder,
    ) -> Option<u32> {
        let config = t.shadow_config();
        if !config.cast_shadow {
            return None;
        }

        let res = g.allocate_texture(
            "shadow map".into(),
            Vec3u::new(config.size.x as u32, config.size.y as u32, 1),
            wgpu::TextureFormat::Depth32Float,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            Some(ClearValue::Depth(1f32)),
            1,
        );

        let mut shadow_pass = RenderPassBuilder::new("phong's light shadow pass");
        shadow_pass.render_target(RenderTargetDescriptor {
            colors: smallvec::smallvec![],
            depth: Some(DepthRenderTargetDescriptor {
                prefer_attachment: PreferAttachment::Resource(res),
                depth_ops: Some(ResourceOps::load_store()),
                stencil_ops: None,
            }),
        });
        shadow_pass.async_execute(Arc::new(Mutex::new(ShadowRenderer {
            shared: shared.clone(),
            light: t.clone(),
            size: config.size,
        })));
        g.add_render_pass(shadow_pass);
        Some(res)
    }
}

impl MaterialRendererFactory for PhongMaterialRendererFactory {
    fn setup(
        &self,
        materials_map: &RenderMaterialPsoBuilder,
        gpu: &core::backends::wgpu_backend::WGPUResource,
        g: &mut core::graph::rdg::RenderGraphBuilder,
        setup_resource: &core::render::material::SetupResource,
    ) {
        let shadow_sampler = Arc::new(gpu.device().create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToBorder,
            address_mode_v: wgpu::AddressMode::ClampToBorder,
            address_mode_w: wgpu::AddressMode::ClampToBorder,
            border_color: Some(wgpu::SamplerBorderColor::OpaqueWhite),
            compare: Some(wgpu::CompareFunction::LessEqual),

            ..Default::default()
        }));

        let lights = setup_resource.scene.get_resource::<SceneLights>().unwrap();
        let any_shadow_cast = lights.any_shadow();

        if any_shadow_cast {
            setup_resource
                .shader_tech_collection
                .setup(
                    gpu.device(),
                    "shadow",
                    &VariantFlags::default(),
                    0,
                    |pass_name| {
                        let mut rdo = RenderDescriptorObject::new();

                        let depth_format = wgpu::TextureFormat::Depth32Float;

                        rdo = rdo.set_depth(depth_format, |depth: &mut _| {
                            depth.depth_compare = wgpu::CompareFunction::Less;
                            depth.depth_write_enabled = true;
                        });
                        rdo
                    },
                )
                .unwrap();
        }

        let mut scene_shared = PhongMaterialSceneSharedData {
            variants_base: vec![],
            variants_add: vec![],
        };

        let lights = setup_resource.scene.get_resource::<SceneLights>().unwrap();
        let mut variants_base = vec![];
        let mut variants_add = vec![];

        let has_direct_light = lights.has_direct_light();

        if has_direct_light {
            variants_base.push("DIRECT_LIGHT");
            let dlight = lights.direct_light().unwrap();
            if dlight.shadow_config().cast_shadow {
                variants_base.push("SHADOW");
            }
            if dlight.shadow_config().pcf {
                variants_base.push("SHADOW_PCF");
            }
        }

        scene_shared.variants_base = variants_base;

        for light in lights.extra_lights() {
            let tag = match light.as_ref() {
                Light::Spot(_s) => "SPOT_LIGHT",
                Light::Point(_p) => "POINT_LIGHT",
                _ => panic!(),
            };
            let mut res = vec![tag];
            if light.shadow_config().cast_shadow {
                res.push("SHADOW");
            }
            if light.shadow_config().pcf {
                res.push("SHADOW_PCF");
            }
            variants_add.push(res);
        }

        scene_shared.variants_add = variants_add;

        let scene_shared = Arc::new(scene_shared);
        let shared = PhongMaterialSharedData {
            mesh_buffer_collector: MeshBufferCollector::new(),
            material_shader_collector: setup_resource.shader_tech_collection.clone(),
            shader_bind_group_collection: ShaderBindGroupCollection::new(
                "phong-render".to_string(),
            ),
            scene_shared: scene_shared.clone(),
        };

        let shared = Arc::new(Mutex::new(shared));

        setup_resource
            .shader_tech_collection
            .setup(
                gpu.device(),
                "phong",
                &VariantFlags::default(),
                0,
                |pass_name| {
                    let mut rdo = RenderDescriptorObject::new();
                    let depth_format = wgpu::TextureFormat::Depth32Float;

                    if pass_name == "phong-forward-base" {
                        rdo = rdo.set_depth(depth_format, |depth: &mut _| {
                            depth.depth_compare = wgpu::CompareFunction::Less;
                            depth.depth_write_enabled = true;
                        });
                    } else {
                        // add
                        rdo = rdo.set_depth(depth_format, |depth: &mut _| {
                            depth.depth_compare = wgpu::CompareFunction::Equal;
                            depth.depth_write_enabled = false;
                        });
                    }
                    rdo
                },
            )
            .unwrap();

        for (layer, _) in &materials_map.map {
            let mut base_pass =
                RenderPassBuilder::new(format!("phong forward base pass layer {}", layer));
            base_pass.default_color_depth_render_target();

            let mut shadow_map_id = None;
            if has_direct_light {
                let res = self.add_shadow_pass_for_light(
                    lights.direct_light().unwrap(),
                    shared.clone(),
                    g,
                );
                if let Some(res) = res {
                    base_pass.read_texture(res);
                    shadow_map_id = Some(res)
                }
            }

            base_pass.async_execute(Arc::new(Mutex::new(base::PhongMaterialBaseRenderer {
                shared: shared.clone(),
                has_shadow_pass: any_shadow_cast
                    && has_direct_light
                    && lights.direct_light().unwrap().shadow_config().cast_shadow,
                shadow_map_sampler: shadow_sampler.clone(),
                shadow_map_binding: None,
                has_direct_light,
                shadow_map_id,
                layer: *layer,
                lights: lights.clone(),
            })));
            g.add_render_pass(base_pass);
            shadow_map_id = None;

            for (index, light) in lights.extra_lights().iter().enumerate() {
                let mut add_pass = RenderPassBuilder::new(format!(
                    "phong forward add pass {} layer {}",
                    index, layer
                ));
                let res = self.add_shadow_pass_for_light(light.clone(), shared.clone(), g);
                if let Some(res) = res {
                    add_pass.read_texture(res);
                    shadow_map_id = Some(res)
                }

                // add pass
                add_pass.default_color_depth_render_target();

                add_pass.async_execute(Arc::new(Mutex::new(base::PhongMaterialAddRenderer {
                    shared: shared.clone(),
                    index,
                    shadow_map_binding: None,
                    shadow_map_sampler: shadow_sampler.clone(),
                    shadow_map_id,
                    has_shadow_pass: light.shadow_config().cast_shadow,
                    layer: *layer,
                    lights: lights.clone(),
                })));

                g.add_render_pass(add_pass);
            }
        }
    }
}

// struct PhongMaterialBufferInstantiation {
//     tech: Arc<ShaderTech>,
//     msaa: u32,
//     scene_shared: Arc<PhongMaterialSceneSharedData>,
// }

// impl MaterialBufferInstantiation for PhongMaterialBufferInstantiation {
//     fn create_pipeline(
//         &self,
//         material: &core::material::Material,
//         global_layout: &wgpu::BindGroupLayout,
//         gpu: &core::backends::wgpu_backend::WGPUResource,
//     ) -> PipelinePassResource {
//         let mut variants = material.face_by::<PhongMaterialFace>().variants.clone();
//         let variants_add = material.face_by::<PhongMaterialFace>().variants_add.clone();
//         variants.extend_from_slice(&self.scene_shared.variants_base);

//         let base_template = self
//             .tech
//             .register_variant_pass(gpu.device(), 0, &variants)
//             .unwrap();

//         let mut passes = vec![];
//         let mut instances = vec![];
//         let mut config = vec![];
//         passes.push(base_template);

//         let mut ins = RenderDescriptorObject::new().set_msaa(self.msaa);
//         ins = ins.add_target(ColorTargetBuilder::new(gpu.surface_format()).build());
//         let depth_format = wgpu::TextureFormat::Depth32Float;
//         ins = ins.set_primitive(|p: &mut _| *p = *material.primitive());
//         ins = ins.set_depth(depth_format, |depth: &mut _| {
//             depth.depth_compare = wgpu::CompareFunction::Less;
//             depth.depth_write_enabled = true;
//             depth.bias = wgpu::DepthBiasState {
//                 constant: 0,
//                 slope_scale: 0.0f32,
//                 clamp: 0.0f32,
//             }
//         });
//         instances.push(ins);
//         config.push(ResolvePipelineConfig {
//             constant_stages: vec![wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT],
//             global_bind_group_layout: Some(global_layout),
//         });

//         for add in &self.scene_shared.variants_add {
//             log::info!("create phong material render with add pass pipeline");
//             let mut variants_add2 = variants_add.clone();
//             variants_add2.extend_from_slice(add);

//             let add_template = self
//                 .tech
//                 .register_variant_pass(gpu.device(), 1, &variants_add2)
//                 .unwrap();

//             passes.push(add_template);

//             let mut ins = RenderDescriptorObject::new().set_msaa(self.msaa);
//             ins = ins.add_target(
//                 ColorTargetBuilder::new(gpu.surface_format())
//                     .set_blender(wgpu::BlendState {
//                         color: wgpu::BlendComponent {
//                             src_factor: wgpu::BlendFactor::One,
//                             dst_factor: wgpu::BlendFactor::One,
//                             operation: wgpu::BlendOperation::Add,
//                         },
//                         alpha: wgpu::BlendComponent::REPLACE,
//                     })
//                     .build(),
//             );
//             let depth_format = wgpu::TextureFormat::Depth32Float;
//             ins = ins.set_primitive(|p: &mut _| *p = *material.primitive());
//             ins = ins.set_depth(depth_format, |depth: &mut _| {
//                 depth.depth_compare = wgpu::CompareFunction::Equal;
//                 depth.depth_write_enabled = false;
//                 depth.bias = wgpu::DepthBiasState {
//                     constant: 0,
//                     slope_scale: 0.0f32,
//                     clamp: 0.0f32,
//                 }
//             });
//             instances.push(ins);
//             config.push(ResolvePipelineConfig {
//                 constant_stages: vec![wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT],
//                 global_bind_group_layout: Some(global_layout),
//             });
//         }

//         resolve_pipeline3(gpu, &passes, &instances, &config)
//     }

//     fn create_bind_group(
//         &self,
//         material: &core::material::Material,
//         buffers: &[wgpu::Buffer],
//         pipeline: &PipelinePassResource,
//         device: &wgpu::Device,
//     ) -> Vec<Option<wgpu::BindGroup>> {
//         let buffer = &buffers[0];

//         let mat = material.face_by::<PhongMaterialFace>();
//         let mut entries = vec![];
//         entries.push(wgpu::BindGroupEntry {
//             binding: 0,
//             resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
//                 buffer,
//                 offset: 0,
//                 size: None,
//             }),
//         });

//         if let Some(sampler) = &mat.sampler {
//             entries.push(wgpu::BindGroupEntry {
//                 binding: entries.len() as u32,
//                 resource: wgpu::BindingResource::Sampler(sampler.sampler()),
//             })
//         }
//         if let Some(texture) = mat.diffuse.texture_ref() {
//             entries.push(wgpu::BindGroupEntry {
//                 binding: entries.len() as u32,
//                 resource: wgpu::BindingResource::TextureView(texture.texture_view()),
//             });
//         }

//         if let Some(texture) = mat.normal.texture_ref() {
//             entries.push(wgpu::BindGroupEntry {
//                 binding: entries.len() as u32,
//                 resource: wgpu::BindingResource::TextureView(texture.texture_view()),
//             });
//         }

//         if let Some(texture) = mat.specular.texture_ref() {
//             entries.push(wgpu::BindGroupEntry {
//                 binding: entries.len() as u32,
//                 resource: wgpu::BindingResource::TextureView(texture.texture_view()),
//             });
//         }

//         if let Some(texture) = mat.emissive.texture_ref() {
//             entries.push(wgpu::BindGroupEntry {
//                 binding: entries.len() as u32,
//                 resource: wgpu::BindingResource::TextureView(texture.texture_view()),
//             });
//         }

//         let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
//             label: Some("phong material bind group"),
//             layout: &pipeline.pass[0].get_bind_group_layout(2),
//             entries: &entries,
//         });

//         let mut bind_groups = vec![];
//         bind_groups.push(Some(bind_group));

//         for (_light, buffer) in self
//             .scene_shared
//             .lights_uniforms
//             .iter()
//             .zip(self.scene_shared.lights_buffer.iter())
//         {
//             let mut light_entries = vec![];

//             light_entries.push(wgpu::BindGroupEntry {
//                 binding: 0,
//                 resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
//                     buffer: buffer,
//                     offset: 0,
//                     size: None,
//                 }),
//             });

//             let light_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
//                 label: Some("light bind group"),
//                 layout: &pipeline.pass[0].get_bind_group_layout(1),
//                 entries: &light_entries,
//             });

//             bind_groups.push(Some(light_bind_group));
//         }

//         bind_groups
//     }
// }
