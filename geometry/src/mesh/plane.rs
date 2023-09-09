use core::{
    mesh::{
        builder::{MeshBuilder, MeshPropertiesBuilder, MeshPropertyType},
        Mesh,
    },
    types::{Color, Vec3f},
};

pub struct PlaneMeshBuilder {
    normal: bool,
    color: bool,
    segments_x: u32,
    segments_z: u32,
    colors: Vec<Color>,
    default_color: Color,
}

impl Default for PlaneMeshBuilder {
    fn default() -> Self {
        Self {
            normal: false,
            color: false,
            segments_x: 1,
            segments_z: 1,
            colors: vec![],
            default_color: Color::default(),
        }
    }
}

impl PlaneMeshBuilder {
    pub fn enable_normal(mut self) -> Self {
        self.normal = true;
        self
    }

    pub fn enable_color(mut self, default_color: Color) -> Self {
        self.color = true;
        self.default_color = default_color;
        self
    }

    pub fn set_color_face_at_index(mut self, index: usize, color: Color) -> Self {
        if self.colors.len() < index {
            self.colors.resize(index + 1, self.default_color);
        }
        self.colors[index] = color;
        self
    }

    pub fn set_segments(mut self, x: u32, z: u32) -> Self {
        self.segments_x = x;
        self.segments_z = z;
        self
    }

    pub fn build(mut self) -> Mesh {
        let mut builder = MeshBuilder::default();
        let mut properties_builder = MeshPropertiesBuilder::default();
        let property = MeshPropertyType::new::<Vec3f>("normal_vertex");
        if self.normal {
            properties_builder.add_property(property);
        }
        let color_property = MeshPropertyType::new::<Color>("color");
        if self.color {
            properties_builder.add_property(color_property);
        }

        let dx = 1f32 / self.segments_x as f32;
        let dz = 1f32 / self.segments_z as f32;

        let x_beg = -0.5f32;
        let z_beg = -0.5f32;

        let mut x_cur = x_beg;
        let mut z_cur = z_beg;
        let mut n = 0;

        let mut vertices = vec![];
        let mut indices = vec![];
        for _ in 0..self.segments_x {
            let x_next = x_cur + dx;
            z_cur = z_beg;
            for _ in 0..self.segments_z {
                let z_next = z_cur + dz;
                // add plane
                vertices.push(Vec3f::new(x_cur, 0f32, z_next));
                vertices.push(Vec3f::new(x_next, 0f32, z_next));
                vertices.push(Vec3f::new(x_cur, 0f32, z_cur));
                vertices.push(Vec3f::new(x_next, 0f32, z_cur));

                indices.extend_from_slice(&[n, n + 1, n + 2, n + 3, n + 2, n + 1]);
                n += 4;

                z_cur = z_next;
            }
            x_cur = x_next;
        }
        builder.add_position_vertices3(&vertices);
        builder.add_indices32(&indices);

        if self.normal {
            let mut normals = vec![];
            for _ in 0..vertices.len() {
                normals.push(Vec3f::new(0f32, 1f32, 0f32));
            }
            properties_builder.add_property_data(property, &normals);
        }

        if self.color {
            if self.colors.len() < vertices.len() {
                self.colors.resize(vertices.len(), self.default_color);
            }
            properties_builder.add_property_data(color_property, &self.colors);
        }

        builder.set_properties(properties_builder.build());

        builder.build().unwrap()
    }
}
