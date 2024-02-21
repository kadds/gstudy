use core::{
    backends::wgpu_backend::WGPUResource,
    context::{RContext, ResourceRef},
    material::{
        basic::BasicMaterialFaceBuilder, InputResource, InputResourceBits, InputResourceBuilder,
        Material, MaterialBuilder,
    },
    mesh::builder::MeshPropertyType,
    render::default_blender,
    types::{Color, Vec2f, Vec3f, Vec4f},
    util::any_as_x_slice_array,
    wgpu,
};
use std::{collections::HashMap, sync::Arc};

use crate::TextureMap;

use super::MaterialLoader;

#[derive(Debug, Hash, Eq, PartialEq)]
enum MaterialMapKey {
    Gltf(usize),
    Default,
}

struct PMaterialMap {
    b: MaterialBuilder,
    fb: BasicMaterialFaceBuilder,
    settler: HashMap<InputResourceBits, Arc<Material>>,
    default_sampler: ResourceRef,
}

impl PMaterialMap {
    pub fn generate_material(
        &mut self,
        additional_input: &InputResource<Color>,
        context: &RContext,
    ) -> Arc<Material> {
        let mut input = self.fb.get_texture();
        input.merge_available(additional_input);
        self.settler
            .entry(input.bits())
            .or_insert_with(|| {
                let mut fb = self.fb.clone().texture(input.clone());
                let b = self.b.clone();
                if input.is_texture() {
                    if !fb.has_sampler() {
                        // add default sampler
                        fb.set_sampler(self.default_sampler.clone());
                    }
                }
                b.face(fb.build()).build(context)
            })
            .clone()
    }
}

pub struct BasicMaterialLoader {
    map: HashMap<MaterialMapKey, PMaterialMap>,
    gpu: Arc<WGPUResource>,
}

impl BasicMaterialLoader {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        let mut map = HashMap::new();

        {
            let mut material_builder = MaterialBuilder::default();
            material_builder = material_builder.primitive(wgpu::PrimitiveState::default());
            material_builder = material_builder.name("default");
            let face_builder = BasicMaterialFaceBuilder::default().texture(
                InputResourceBuilder::only_constant(Color::new(1f32, 1f32, 0.8f32, 1f32)),
            );

            map.insert(
                MaterialMapKey::Default,
                PMaterialMap {
                    b: material_builder,
                    fb: face_builder,
                    settler: HashMap::default(),
                    default_sampler: gpu.default_sampler(),
                },
            );
        }
        Self { map, gpu }
    }
}

impl MaterialLoader for BasicMaterialLoader {
    fn load_material(
        &mut self,
        index: usize,
        material: &gltf::Material,
        texture_map: &TextureMap,
        samplers: &[ResourceRef],
    ) -> anyhow::Result<()> {
        let mut primitive = wgpu::PrimitiveState::default();
        let mut material_builder = MaterialBuilder::default();
        if material.double_sided() {
            primitive.cull_mode = Some(wgpu::Face::Back);
        }
        material_builder.set_primitive(primitive);
        material_builder.set_name(material.name().unwrap_or_default());

        let mut face_builder = BasicMaterialFaceBuilder::default();

        let texture = material.pbr_metallic_roughness().base_color_texture();
        let mut input_resource = InputResourceBuilder::new();

        if let Some(tex) = texture {
            let texture_index = tex.texture().index();
            let (sampler_index, texture) = texture_map.get(&texture_index).unwrap();
            input_resource.add_texture(texture.clone());
            if let Some(index) = sampler_index {
                face_builder.set_sampler(samplers[*index].clone());
            } else {
                // use default
                face_builder.set_sampler(self.gpu.default_sampler());
            }
        }

        let color = material.pbr_metallic_roughness().base_color_factor();
        input_resource.add_constant(color.into());
        let input = input_resource.build();

        face_builder.set_texture(input);

        match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => {}
            gltf::material::AlphaMode::Mask => {
                face_builder.set_alpha_test(material.alpha_cutoff().unwrap_or(0.5f32));
            }
            gltf::material::AlphaMode::Blend => {
                material_builder.set_blend(default_blender());
            }
        }
        self.map.insert(
            MaterialMapKey::Gltf(index),
            PMaterialMap {
                b: material_builder,
                fb: face_builder,
                settler: HashMap::new(),
                default_sampler: self.gpu.default_sampler(),
            },
        );

        Ok(())
    }
    fn load_properties_vertices(
        &mut self,
        p: &gltf::Primitive,
        mesh_builder: &mut core::mesh::builder::MeshBuilder,
        mesh_properties_builder: &mut core::mesh::builder::MeshPropertiesBuilder,
        buf_view: &crate::GltfBufferView,
        res: &mut crate::GltfSceneInfo,
    ) -> anyhow::Result<Arc<Material>> {
        let mut has_color = false;
        let mut has_texture = false;
        let color_property = MeshPropertyType::new::<Color>("color");
        let texture_property = MeshPropertyType::new::<Vec2f>("texture");

        for (semantic, _) in p.attributes() {
            match semantic {
                gltf::Semantic::Colors(_) => {
                    has_color = true;
                }
                gltf::Semantic::TexCoords(_) => {
                    has_texture = true;
                }
                _ => (),
            }
        }
        if has_color {
            mesh_properties_builder.add_property(color_property);
        }
        if has_texture {
            mesh_properties_builder.add_property(texture_property);
        }

        for (semantic, accessor) in p.attributes() {
            match semantic {
                gltf::Semantic::Extras(ext) => {
                    log::info!("ignore extra {}", ext);
                }
                gltf::Semantic::Positions => {
                    let buf = buf_view.buffer[0].read_bytes_from_accessor(&accessor);
                    match accessor.data_type() {
                        gltf::accessor::DataType::F32 => {}
                        _ => {
                            anyhow::bail!("position invalid data type");
                        }
                    };
                    match accessor.dimensions() {
                        gltf::accessor::Dimensions::Vec3 => {
                            let data: &[Vec3f] = any_as_x_slice_array(buf);
                            res.total_vertices += data.len() as u64;
                            mesh_builder.add_position_vertices3(data);
                        }
                        _ => {
                            anyhow::bail!("position should be vec3f");
                        }
                    };
                }
                gltf::Semantic::Normals => {}
                gltf::Semantic::Tangents => {}
                gltf::Semantic::Colors(_index) => {
                    if !has_color {
                        continue;
                    }

                    let buf = buf_view.buffer[0].read_bytes_from_accessor(&accessor);
                    match accessor.data_type() {
                        gltf::accessor::DataType::F32 => {}
                        _ => {
                            anyhow::bail!("color invalid data type");
                        }
                    };
                    match accessor.dimensions() {
                        gltf::accessor::Dimensions::Vec4 => {
                            let data: &[Vec4f] = any_as_x_slice_array(buf);
                            mesh_properties_builder.add_property_data(color_property, data);
                        }
                        gltf::accessor::Dimensions::Vec3 => {
                            let data: &[Vec3f] = any_as_x_slice_array(buf);
                            let mut trans_data = Vec::new();
                            for block in data {
                                trans_data.push(Vec4f::new(block[0], block[1], block[2], 1f32));
                            }
                            mesh_properties_builder.add_property_data(color_property, &trans_data);
                        }
                        _ => {
                            anyhow::bail!("color should be vec3f/vec4f");
                        }
                    };
                    has_color = false;
                }
                gltf::Semantic::TexCoords(_index) => {
                    if !has_texture {
                        continue;
                    }

                    let buf = buf_view.buffer[0].read_bytes_from_accessor(&accessor);
                    match accessor.data_type() {
                        gltf::accessor::DataType::F32 => {}
                        _ => {
                            anyhow::bail!("texcoord invalid data type");
                        }
                    };
                    match accessor.dimensions() {
                        gltf::accessor::Dimensions::Vec2 => {}
                        _ => {
                            anyhow::bail!("texcoord should be vec2f");
                        }
                    };

                    let f = any_as_x_slice_array(buf);
                    let mut data = Vec::new();
                    for block in f.chunks(2) {
                        data.push(Vec2f::new(block[0], block[1]));
                    }

                    mesh_properties_builder.add_property_data(texture_property, &data);
                    has_texture = false;
                }
                gltf::Semantic::Joints(_index) => {}
                gltf::Semantic::Weights(_index) => {}
            }
        }

        let idx = p.material().index();
        let key = if let Some(idx) = idx {
            MaterialMapKey::Gltf(idx)
        } else {
            MaterialMapKey::Default
        };
        let mut input = InputResourceBuilder::new();
        if mesh_properties_builder.has_property(&color_property) {
            // vertex color
            input.add_pre_vertex();
        }
        if mesh_properties_builder.has_property(&texture_property) {
            // vertex color
            input.add_texture(self.gpu.default_texture());
        }

        let material = self
            .map
            .get_mut(&key)
            .ok_or(anyhow::anyhow!("material not found {:?}", key))?
            .generate_material(&input.build(), self.gpu.context());

        Ok(material)
    }
    fn load_light(
        &self,
        light: &gltf::khr_lights_punctual::Light,
        scene: &core::scene::Scene,
    ) -> anyhow::Result<()> {
        log::info!("ignore light {}", light.name().unwrap_or_default());
        Ok(())
    }
}
