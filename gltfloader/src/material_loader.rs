use core::{
    context::ResourceRef,
    material::Material,
    mesh::builder::{MeshBuilder, MeshPropertiesBuilder},
};
use std::sync::Arc;

use crate::{GltfBufferView, LoadResult, TextureMap};

pub trait MaterialLoader {
    fn load_material(
        &mut self,
        index: usize,
        material: &gltf::Material,
        texture_map: &TextureMap,
        samplers: &[ResourceRef],
    ) -> anyhow::Result<()>;
    fn load_properties_vertices(
        &self,
        p: &gltf::Primitive,
        mesh_builder: &mut MeshBuilder,
        mesh_properties_builder: &mut MeshPropertiesBuilder,
        buf_view: &GltfBufferView,
        res: &mut LoadResult,
    ) -> anyhow::Result<Arc<Material>>;
}

pub mod basic_loader;
