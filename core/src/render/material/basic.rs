use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use wgpu::util::DeviceExt;

use crate::{
    backends::wgpu_backend::{GpuInputMainBuffersWithProps, ResourceOps, WGPUResource},
    graph::rdg::{
        backend::{GraphCopyEngine, GraphRenderEngine},
        pass::*,
        RenderPassBuilder,
    },
    material::{basic::*, Material, MaterialFace, MaterialId},
    mesh::Mesh,
    render::{
        common::{FramedCache, StaticMeshMerger},
        resolve_pipeline, ColorTargetBuilder, PipelinePassResource, RenderDescriptorObject,
    },
    types::*,
    util::any_as_u8_slice,
};

use super::{take_rs, MaterialRendererFactory, RenderMaterialContext, RenderSource, SetupResource};

pub struct MaterialGpuResource {
    global_bind_group: wgpu::BindGroup,

    material_bind_buffers: FramedCache<MaterialId, wgpu::Buffer>,
    bind_groups: FramedCache<MaterialId, wgpu::BindGroup>,

    template: Arc<Vec<tshader::Pass>>,
    pipeline: PipelinePassResource,
}

struct MeshMerger {
    static_mesh_merger: StaticMeshMerger,
    dynamic_mesh_buffer: GpuInputMainBuffersWithProps,
}

struct ObjectBuffer {
    index: Option<wgpu::Buffer>,
    vertex: wgpu::Buffer,
    vertex_properties: Option<wgpu::Buffer>,
}

struct BasicMaterialHardwareRendererInner {
    material_pipelines_cache: FramedCache<u64, MaterialGpuResource>,

    static_object_buffers: FramedCache<u64, ObjectBuffer>,
    dynamic_object_buffers: HashMap<u64, ObjectBuffer>,

    tech: Arc<tshader::ShaderTech>,
}

pub struct BasicMaterialHardwareRenderer {
    inner: BasicMaterialHardwareRendererInner,
}

fn create_materia_buffer(material: &Material, gpu: &WGPUResource) -> wgpu::Buffer {
    let mat = material.face_by::<BasicMaterialFace>();
    if let Some(alpha) = material.alpha_test() {
        let buf = gpu.new_wvp_buffer::<ConstParameterWithAlpha>(Some("basic"));
        let w = ConstParameterWithAlpha {
            color: mat.color(),
            alpha,
            _pad: Vec3f::zeros(),
        };
        gpu.queue().write_buffer(&buf, 0, any_as_u8_slice(&w));
        buf
    } else {
        let buf = gpu.new_wvp_buffer::<ConstParameter>(Some("basic"));
        let w = ConstParameter { color: mat.color() };
        gpu.queue().write_buffer(&buf, 0, any_as_u8_slice(&w));
        buf
    }
}

fn create_static_object_buffer(id: u64, mesh: &Mesh, device: &wgpu::Device) -> ObjectBuffer {
    let index = if let Some(index) = mesh.indices_view() {
        Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} index buffer", id)),
                contents: index,
                usage: wgpu::BufferUsages::INDEX,
            }),
        )
    } else {
        None
    };

    let vertex = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{} vertex buffer", id)),
        contents: mesh.vertices_view().unwrap_or_default(),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let vertex_properties = if mesh.properties_view().len() > 0 {
        Some(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{} properties buffer", id)),
                contents: mesh.properties_view(),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        )
    } else {
        None
    };

    ObjectBuffer {
        index,
        vertex,
        vertex_properties,
    }
}

impl RenderPassExecutor for BasicMaterialHardwareRenderer {
    fn prepare<'a>(
        &'a mut self,
        context: RenderPassContext<'a>,
        engine: &mut GraphCopyEngine,
    ) -> Option<()> {
        self.inner.dynamic_object_buffers.clear();

        let rs = take_rs::<BasicMaterialFace>(&context)?;
        let c = rs.scene.get_container();

        self.inner.material_pipelines_cache.recall();

        for layer in &rs.list {
            for indirect in &layer.material {
                let material = indirect.material.as_ref();
                log::info!(
                    "[{:?}], material {:?} {}",
                    &layer.objects(&indirect),
                    material.id(),
                    material.is_transparent()
                );
                let mgr = self.prepare_material_pipeline(&rs.gpu, &layer.main_camera, material);
                // create uniform buffer
                mgr.material_bind_buffers.get_or(material.id(), |_| {
                    create_materia_buffer(material, engine.gpu())
                });

                // create index/vertex buffer
                let objects = layer.objects(indirect);

                for id in objects {
                    let obj = match c.get(id) {
                        Some(v) => v,
                        None => continue,
                    };
                    let obj = obj.o();
                    let mesh = obj.geometry().mesh();

                    if obj.geometry().is_static() {
                        self.inner.static_object_buffers.get_or(*id, |_| {
                            create_static_object_buffer(*id, &mesh, engine.device())
                        });
                    } else {
                        self.inner
                            .dynamic_object_buffers
                            .entry(*id)
                            .or_insert_with(|| {
                                create_static_object_buffer(*id, &mesh, engine.device())
                            });
                    }
                }
            }
        }

        Some(())
    }

    fn queue<'b>(&'b mut self, context: RenderPassContext<'b>, device: &wgpu::Device) {
        let rs = take_rs::<BasicMaterialFace>(&context).unwrap();

        for layer in &rs.list {
            for indirect in &layer.material {
                let material = indirect.material.as_ref();
                let mgr = self.prepare_material_pipeline(&rs.gpu, &layer.main_camera, material);
                let mat = material.face_by::<BasicMaterialFace>();
                mgr.bind_groups.get_or(material.id(), |_| {
                    let b = mgr.material_bind_buffers.get_mut(&material.id()).unwrap();
                    let mut entries = vec![];
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                            buffer: &b,
                            offset: 0,
                            size: None,
                        }),
                    });

                    if let Some(texture) = mat.texture() {
                        let sampler = mat.sampler().unwrap();
                        entries.push(wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(sampler.sampler()),
                        });
                        entries.push(wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(texture.texture_view()),
                        });
                    }

                    device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("basic material"),
                        layout: &mgr.pipeline.pass[0].get_bind_group_layout(1),
                        entries: &entries,
                    })
                });
            }
        }
    }

    fn render<'a>(&'a mut self, context: RenderPassContext<'a>, engine: &mut GraphRenderEngine) {
        let rs = take_rs::<BasicMaterialFace>(&context).unwrap();
        let c = rs.scene.get_container();

        for layer in &rs.list {
            let mut pass = engine.begin(layer.layer);

            for indirect in &layer.material {
                let objects = layer.objects(indirect);
                let material = indirect.material.as_ref();

                let key = material.hash_key();
                let mgr = self.inner.material_pipelines_cache.get(&key).unwrap();
                let material_bind_group = mgr.bind_groups.get(&material.id()).unwrap();

                pass.set_pipeline(&mgr.pipeline.pass[0].render());

                pass.set_bind_group(0, &mgr.global_bind_group, &[]); // camera bind group
                pass.set_bind_group(1, &material_bind_group, &[]); // material bind group

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
                        wgpu::ShaderStages::all(),
                        0,
                        any_as_u8_slice(object_uniform.mat()),
                    );

                    let b = if obj.geometry().is_static() {
                        self.inner.static_object_buffers.get(id).unwrap()
                    } else {
                        self.inner.dynamic_object_buffers.get(id).unwrap()
                    };
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
                    if let Some(_) = &b.index {
                        pass.draw_indexed(0..mesh.index_count().unwrap(), 0, 0..1);
                    } else {
                        pass.draw(0..mesh.vertex_count() as u32, 0..1);
                    }
                    pass.pop_debug_group();
                }
            }
        }
    }

    fn cleanup<'b>(&'b mut self, context: RenderPassContext<'b>) {
        let rs = take_rs::<BasicMaterialFace>(&context).unwrap();
    }
}

impl BasicMaterialHardwareRenderer {
    pub fn prepare_material_pipeline(
        &mut self,
        gpu: &WGPUResource,
        camera: &wgpu::Buffer,
        material: &Material,
    ) -> &mut MaterialGpuResource {
        let label = self.label();
        let inner = &mut self.inner;

        let key = material.hash_key();
        inner.material_pipelines_cache.get_mut_or(key, |_| {
            let variants = material.face_by::<BasicMaterialFace>().variants();
            let template = inner.tech.register_variant(gpu.device(), variants).unwrap();

            let mut ins = RenderDescriptorObject::new();

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

            let pipeline = resolve_pipeline(gpu, template.clone(), ins);

            let global_bind_group = gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
                label,
                layout: &pipeline.pass[0].get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: camera,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

            MaterialGpuResource {
                template,
                pipeline,
                bind_groups: FramedCache::new(),
                material_bind_buffers: FramedCache::new(),
                global_bind_group,
            }
        })
    }
    pub fn label(&self) -> Option<&'static str> {
        Some("basic material")
    }
}

#[derive(Default)]
pub struct BasicMaterialRendererFactory {}

impl MaterialRendererFactory for BasicMaterialRendererFactory {
    fn setup(
        &self,
        materials: &[Arc<Material>],
        gpu: &WGPUResource,
        g: &mut crate::graph::rdg::RenderGraphBuilder,
        setup_resource: &SetupResource,
    ) {
        let tech = setup_resource
            .shader_loader
            .load_tech("basic_forward")
            .unwrap();

        let r = Arc::new(Mutex::new(BasicMaterialHardwareRenderer {
            inner: BasicMaterialHardwareRendererInner {
                tech,
                material_pipelines_cache: FramedCache::new(),
                static_object_buffers: FramedCache::new(),
                dynamic_object_buffers: HashMap::new(),
            },
        }));

        let mut pass = RenderPassBuilder::new("basic render pass");
        pass.render_target(RenderTargetDescriptor {
            colors: smallvec::smallvec![ColorRenderTargetDescriptor {
                prefer_attachment: PreferAttachment::Default,
                ops: ResourceOps {
                    load: None,
                    store: true,
                },
            }],
            depth: Some(DepthRenderTargetDescriptor {
                prefer_attachment: PreferAttachment::Default,
                depth_ops: Some(ResourceOps {
                    load: None,
                    store: true,
                }),
                stencil_ops: None,
            }),
        });
        pass.async_execute(r.clone());
        pass.add_constraint(PassConstraint::Last);

        g.add_render_pass(pass);
    }
}
