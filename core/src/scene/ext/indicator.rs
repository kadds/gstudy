use std::sync::Arc;

use wgpu::PrimitiveState;

use crate::{
    context::RContext,
    material::{basic::BasicMaterialFaceBuilder, MaterialBuilder},
    mesh::{builder::MeshBuilder, StaticGeometry},
    scene::RenderObject,
    types::{Vec3f, Vec4f},
};

pub struct IndicatorBuilder {}

impl IndicatorBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(self, context: &RContext) -> RenderObject {
        let mut mesh_builder = MeshBuilder::new();
        mesh_builder.add_property(crate::mesh::MeshPropertyType::Color);
        let r = Vec4f::new(1f32, 0f32, 0f32, 1f32);
        let g = Vec4f::new(0f32, 1f32, 0f32, 1f32);
        let b = Vec4f::new(0f32, 0f32, 1f32, 1f32);

        let len = 10000f32;

        let z0 = Vec3f::zeros();
        let x = Vec3f::new(len, 0f32, 0f32);
        let y = Vec3f::new(0f32, len, 0f32);
        let z = Vec3f::new(0f32, 0f32, len);

        mesh_builder.add_position_vertices3(&[z0, x, z0, y, z0, z]);
        // mesh_builder.add_indices16(&[0, 1, 2, 3, 4, 5]);
        mesh_builder.add_indices_none();
        mesh_builder
            .add_property_vertices(crate::mesh::MeshPropertyType::Color, &[r, r, g, g, b, b]);

        let mesh = mesh_builder.build().unwrap();

        let face = BasicMaterialFaceBuilder::new().with_color().build();
        let m = MaterialBuilder::default()
            .with_face(face)
            .with_primitive(PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            })
            .build(context);

        RenderObject::new(Box::new(StaticGeometry::new(Arc::new(mesh))), m)
    }
}
