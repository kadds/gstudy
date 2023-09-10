use core::{
    context::ResourceRef,
    material::Material,
    mesh::builder::{MeshBuilder, MeshPropertiesBuilder},
    scene::Scene,
};
use std::sync::Arc;

use crate::{GltfBufferView, GltfSceneInfo, TextureMap};

pub trait MaterialLoader {
    fn load_material(
        &mut self,
        index: usize,
        material: &gltf::Material,
        texture_map: &TextureMap,
        samplers: &[ResourceRef],
    ) -> anyhow::Result<()>;
    fn load_properties_vertices(
        &mut self,
        p: &gltf::Primitive,
        mesh_builder: &mut MeshBuilder,
        mesh_properties_builder: &mut MeshPropertiesBuilder,
        buf_view: &GltfBufferView,
        res: &mut GltfSceneInfo,
    ) -> anyhow::Result<Arc<Material>>;
    fn load_light(
        &self,
        light: &gltf::khr_lights_punctual::Light,
        scene: &Scene,
    ) -> anyhow::Result<()>;
    fn post_load(&mut self, scene: &Scene, info: &GltfSceneInfo) -> anyhow::Result<()> {
        Ok(())
    }
}

pub mod basic_loader;

#[cfg(feature = "phong")]
pub mod phong_loader;
