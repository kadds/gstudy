use core::{
    backends::wgpu_backend::WGPUResource,
    context::ResourceRef,
    material::{basic::BasicMaterialFaceBuilder, Material, MaterialBuilder},
    mesh::builder::MeshPropertyType,
    render::default_blender,
    types::{Color, Vec2f, Vec3f, Vec4f},
    util::any_as_x_slice_array,
};
use std::{collections::HashMap, sync::Arc};

use crate::TextureMap;

use super::MaterialLoader;

#[derive(Debug, Hash, Eq, PartialEq)]
enum MaterialMapKey {
    Gltf(usize),
    Default,
    DefaultWithVertexColor,
}

pub struct BasicMaterialLoader {
    map: HashMap<MaterialMapKey, Arc<Material>>,
    gpu: Arc<WGPUResource>,
}

impl BasicMaterialLoader {
    pub fn new(gpu: Arc<WGPUResource>) -> Self {
        let mut map = HashMap::new();

        {
            let mut material_builder = MaterialBuilder::default();
            material_builder = material_builder.primitive(wgpu::PrimitiveState::default());
            material_builder = material_builder.name("default");
            let basic_material_builder = BasicMaterialFaceBuilder::default().texture(
                core::material::MaterialMap::Constant(Color::new(1f32, 1f32, 0.8f32, 1f32)),
            );
            material_builder = material_builder.face(basic_material_builder.build());

            map.insert(
                MaterialMapKey::Default,
                material_builder.build(&gpu.context()),
            );
        }
        {
            let mut material_builder = MaterialBuilder::default();
            material_builder = material_builder.primitive(wgpu::PrimitiveState::default());
            material_builder = material_builder.name("default vertex color");
            let basic_material_builder =
                BasicMaterialFaceBuilder::default().texture(core::material::MaterialMap::PreVertex);
            material_builder = material_builder.face(basic_material_builder.build());

            map.insert(
                MaterialMapKey::DefaultWithVertexColor,
                material_builder.build(&gpu.context()),
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
        material_builder = material_builder
            .primitive(primitive)
            .name(material.name().unwrap_or_default());

        let mut basic_material_builder = BasicMaterialFaceBuilder::default();

        let texture = material.pbr_metallic_roughness().base_color_texture();
        if let Some(tex) = texture {
            let texture_index = tex.texture().index();
            let (sampler_index, texture) = texture_map.get(&texture_index).unwrap();
            basic_material_builder = basic_material_builder
                .texture(core::material::MaterialMap::Texture(texture.clone()));
            if let Some(index) = sampler_index {
                basic_material_builder = basic_material_builder.sampler(samplers[*index].clone());
            } else {
                // use default
                basic_material_builder = basic_material_builder.sampler(self.gpu.default_sampler());
            }
        } else {
            let color = material.pbr_metallic_roughness().base_color_factor();
            basic_material_builder =
                basic_material_builder.texture(core::material::MaterialMap::Constant(color.into()));
        }

        match material.alpha_mode() {
            gltf::material::AlphaMode::Opaque => {}
            gltf::material::AlphaMode::Mask => {
                basic_material_builder.alpha_test(material.alpha_cutoff().unwrap_or(0.5f32));
            }
            gltf::material::AlphaMode::Blend => {
                material_builder = material_builder.blend(default_blender());
            }
        }
        material_builder = material_builder.face(basic_material_builder.build());
        self.map.insert(
            MaterialMapKey::Gltf(index),
            material_builder.build(self.gpu.context()),
        );

        Ok(())
    }
    fn load_properties_vertices(
        &self,
        p: &gltf::Primitive,
        mesh_builder: &mut core::mesh::builder::MeshBuilder,
        mesh_properties_builder: &mut core::mesh::builder::MeshPropertiesBuilder,
        buf_view: &crate::GltfBufferView,
        res: &mut crate::LoadResult,
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
                    log::info!("extra {}", ext);
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
            if mesh_properties_builder.has_property(&color_property) {
                MaterialMapKey::DefaultWithVertexColor
            } else {
                MaterialMapKey::Default
            }
        };

        let material = self
            .map
            .get(&key)
            .ok_or(anyhow::anyhow!("material not found {:?}", key))?;

        Ok(material.clone())
    }
}
