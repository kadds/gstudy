use std::sync::Arc;

use crate::{
    context::RContext,
    material::{
        basic::{BasicMaterialFace, BasicMaterialFaceBuilder},
        InputResourceBuilder, Material, MaterialBuilder,
    },
    mesh::Mesh,
    types::Color,
};

pub trait DebugMeshGenerator {
    fn generate(&self, color: Color) -> Arc<Mesh>;
}

pub fn new_debug_material(context: &RContext) -> Arc<Material> {
    let fb = BasicMaterialFaceBuilder::new().texture(InputResourceBuilder::only_pre_vertex());
    MaterialBuilder::default()
        .name("debug material")
        .face(fb.build())
        .primitive(wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        })
        .build(context)
}
