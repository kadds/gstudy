use std::sync::Arc;

use wgpu::PrimitiveState;

use crate::{
    context::RContext,
    geometry::{MeshBuilder, StaticGeometry},
    material::{basic::BasicMaterialFaceBuilder, MaterialBuilder},
    scene::RenderObject,
    types::{Vec3f, Vec4f},
    util::any_as_u8_slice_array,
};

pub struct IndicatorBuilder {}

impl IndicatorBuilder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn build(self, context: &RContext) -> RenderObject {
        let mut mesh_builder = MeshBuilder::new();
        mesh_builder.add_props(crate::geometry::MeshCoordType::Color);
        let mut data_builder = mesh_builder.finish_props();
        let r = Vec4f::new(1f32, 0f32, 0f32, 1f32);
        let g = Vec4f::new(0f32, 1f32, 0f32, 1f32);
        let b = Vec4f::new(0f32, 0f32, 1f32, 1f32);

        let len = 10000f32;

        let z0 = Vec3f::zeros();
        let x = Vec3f::new(len, 0f32, 0f32);
        let y = Vec3f::new(0f32, len, 0f32);
        let z = Vec3f::new(0f32, 0f32, len);

        data_builder.add_vertices_position(&[z0, x, z0, y, z0, z]);
        data_builder.add_indices(&[0, 1, 2, 3, 4, 5]);
        data_builder.add_vertices_prop(
            crate::geometry::MeshCoordType::Color,
            any_as_u8_slice_array(&[r, r, g, g, b, b]),
            16,
        );

        let mesh = data_builder.build();
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
