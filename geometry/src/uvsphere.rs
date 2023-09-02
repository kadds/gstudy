use core::{
    mesh::{builder::MeshBuilder, Mesh},
    types::{Color, Vec3f},
};

pub struct UVSphereBuilder {
    normal: bool,
    color: bool,
    default_color: Color,
    segments_u: u32,
    segments_v: u32,
}

impl Default for UVSphereBuilder {
    fn default() -> Self {
        Self {
            normal: false,
            color: false,
            segments_u: 8,
            segments_v: 12,
            default_color: Color::default(),
        }
    }
}

impl UVSphereBuilder {
    pub fn enable_normal(mut self) -> Self {
        self.normal = true;
        self
    }

    pub fn enable_color(mut self, default_color: Color) -> Self {
        self.color = true;
        self.default_color = default_color;
        self
    }

    // u: slices: xz
    // v: stacks: y
    pub fn set_segments(mut self, u: u32, v: u32) -> Self {
        self.segments_u = u;
        self.segments_v = v;
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

        let mut vertices = vec![];
        let mut indices = vec![];
        let mut normals = vec![];
        vertices.push(Vec3f::new(0f32, 0.5f32, 0f32));
        normals.push(Vec3f::new(0f32, 0.5f32, 0f32).normalize());

        for i in 0..self.segments_v {
            let phi = std::f32::consts::PI * (i + 1) as f32 / self.segments_v as f32;
            for j in 0..self.segments_u {
                let theta = 2f32 * std::f32::consts::PI * j as f32 / self.segments_u as f32;
                let x = phi.sin() * theta.cos() * 0.5;
                let y = phi.cos() * 0.5;
                let z = phi.sin() * theta.sin() * 0.5;

                vertices.push(Vec3f::new(x, y, z));
                if self.normal {
                    normals.push(Vec3f::new(x, y, z).normalize());
                }
            }
        }

        vertices.push(Vec3f::new(0f32, -0.5f32, 0f32));
        normals.push(Vec3f::new(0f32, -0.5f32, 0f32).normalize());

        for i in 0..self.segments_u {
            let idx0 = (i + 1) % self.segments_u + 1;
            let idx1 = i + 1;
            indices.extend_from_slice(&[0, idx0, idx1]);

            let idx3 = (i + 1) % self.segments_u + 1 + self.segments_u * (self.segments_v - 2);
            let idx2 = i + 1 + self.segments_u * (self.segments_v - 2);
            indices.extend_from_slice(&[vertices.len() as u32 - 1, idx2, idx3]);
        }

        for i in 0..self.segments_v - 2 {
            let base0 = (i + 1) * self.segments_u + 1;
            let base1 = i * self.segments_u + 1;

            for j in 0..self.segments_u {
                let idx0 = base0 + j;
                let idx1 = base0 + (j + 1) % self.segments_u;
                let idx2 = base1 + j;
                let idx3 = base1 + (j + 1) % self.segments_u;

                indices.extend_from_slice(&[idx0, idx3, idx1, idx2, idx3, idx0]);
            }
        }

        builder.add_position_vertices3(&vertices);
        builder.add_indices32(&indices);

        if self.normal {
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
