
use crate::{
    context::RContext,
    material::{
        basic::BasicMaterialFaceBuilder, input::InputResourceBuilder, MaterialBuilder, MaterialArc
    },
    mesh::Mesh,
    types::Color,
};

pub trait DebugMeshGenerator {
    fn generate(&self, color: Color) -> Mesh;
}

pub fn new_debug_material(context: &RContext) -> MaterialArc {
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
