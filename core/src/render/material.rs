use std::{any::TypeId, collections::{BTreeMap, HashMap}, fmt::Debug, sync::Arc};

use crate::{
    backends::wgpu_backend::WGPUResource,
    graph::rdg::{pass::RenderPassContext, RenderGraphBuilder},
    material::{Material, MaterialFace, MaterialId},
    scene::{LayerId, Scene},
};

use super::GlobalUniform;

pub struct RenderSourceIndirectObjects {
    pub material: Arc<Material>,
    pub mat_id: MaterialId,
    pub offset: usize,
    pub count: usize,
}

impl Debug for RenderSourceIndirectObjects {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSourceIndirectObjects")
            .field("material", &self.material.id())
            .field("mat_id", &self.mat_id)
            .field("offset", &self.offset)
            .field("count", &self.count)
            .finish()
    }
}

pub struct RenderSourceLayer {
    pub objects: Vec<u64>,
    pub material: Vec<RenderSourceIndirectObjects>,
    pub main_camera: Arc<GlobalUniform>,
    pub layer: u32,
}

impl Debug for RenderSourceLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSourceLayer")
            .field("objects", &self.objects)
            .field("material", &self.material)
            .field("layer", &self.layer)
            .finish()
    }
}

impl RenderSourceLayer {
    pub fn objects(&self, r: &RenderSourceIndirectObjects) -> &[u64] {
        &self.objects[r.offset..(r.offset + r.count)]
    }
}

pub struct RenderSource {
    pub gpu: Arc<WGPUResource>,
    pub scene: Arc<Scene>,
    pub list: Vec<RenderSourceLayer>,
    pub layer_map_index: HashMap<LayerId, usize>,
}

impl RenderSource {
    pub fn layer(&self, layer: LayerId) -> &RenderSourceLayer {
        &self.list[self.layer_map_index.get(&layer).cloned().unwrap()]
    }
}

impl Debug for RenderSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderSource")
            .field("list", &self.list)
            .finish()
    }
}

pub struct RenderMaterialContext {
    pub map: HashMap<TypeId, RenderSource>,
}

pub struct SetupResource<'a> {
    pub ui_camera: Arc<GlobalUniform>,
    pub main_camera: Arc<GlobalUniform>,
    pub shader_loader: &'a tshader::Loader,
    pub scene: &'a Scene,
    pub msaa: u32,
}

pub type RenderMaterialBuilderMap = BTreeMap<LayerId, Vec<Arc<Material>>>;

pub trait MaterialRendererFactory {
    fn setup(
        &self,
        materials_map: &RenderMaterialBuilderMap,
        gpu: &WGPUResource,
        g: &mut RenderGraphBuilder,
        setup_resource: &SetupResource,
    );
}

pub mod basic;

pub struct HardwareMaterialShaderResource {
    pub pass: smallvec::SmallVec<[Arc<wgpu::RenderPipeline>; 1]>,
}

pub fn take_rs<'a, T: MaterialFace>(
    context: &'a RenderPassContext<'a>,
) -> Option<&'a RenderSource> {
    let rc = context.take::<RenderMaterialContext>();
    rc.map.get(&std::any::TypeId::of::<T>())
}
