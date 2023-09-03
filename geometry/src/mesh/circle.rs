use core::{
    mesh::{builder::MeshBuilder, Mesh},
    types::{Color, Vec2f, Vec3f},
};

pub struct CircleMeshBuilder {
    normal: bool,
    color: bool,
    default_color: Color,
    segments: u32,
}

impl Default for CircleMeshBuilder {
    fn default() -> Self {
        Self {
            normal: false,
            color: false,
            default_color: Color::default(),
            segments: 32,
        }
    }
}

impl CircleMeshBuilder {
    pub fn enable_normal(mut self) -> Self {
        self.normal = true;
        self
    }

    pub fn enable_color(mut self, default_color: Color) -> Self {
        self.color = true;
        self.default_color = default_color;
        self
    }

    pub fn set_segments(mut self, segments: u32) -> Self {
        self.segments = segments.max(3);
        self
    }

    pub fn build(self) -> Mesh {
        let mut builder = MeshBuilder::new();
        let property = core::mesh::MeshPropertyType::new::<Vec3f>("normal_vertex");
        if self.normal {
            builder.add_property(property);
        }
        let color_property = core::mesh::MeshPropertyType::new::<Color>("color");
        if self.color {
            builder.add_property(color_property);
        }

        let theta_inv = std::f32::consts::PI * 2f32 / self.segments as f32;

        let mut theta = 0f32;
        let mut vertices = vec![];

        vertices.push(Vec3f::zeros());
        let mut indices = vec![];

        for segment in 0..self.segments {
            let x = theta.cos();
            let z = theta.sin();
            theta += theta_inv;
            vertices.push(Vec3f::new(x, 0f32, z));
            let a = (segment + 1) % self.segments + 1;
            let b = segment % self.segments + 1;
            indices.extend_from_slice(&[0, a, b]);
        }

        builder.add_position_vertices3(&vertices);
        builder.add_indices32(&indices);

        if self.normal {
            let mut normals = vec![];
            normals.resize(vertices.len(), Vec3f::new(0f32, 1f32, 0f32));
            builder.add_property_vertices(property, &normals);
        }

        if self.color {
            let mut colors = vec![];
            colors.resize(vertices.len(), self.default_color);
            builder.add_property_vertices(color_property, &colors);
        }

        builder.build().unwrap()
    }
}
