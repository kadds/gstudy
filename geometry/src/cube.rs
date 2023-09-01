use core::{
    mesh::{builder::MeshBuilder, Mesh},
    scene::{Transform, TransformBuilder},
    types::{Color, Vec3f},
};

pub struct CubeMeshBuilder {
    normal: bool,
    color: bool,
    colors: Vec<Color>,
}

impl Default for CubeMeshBuilder {
    fn default() -> Self {
        Self {
            normal: false,
            color: false,
            colors: vec![],
        }
    }
}

impl CubeMeshBuilder {
    pub fn enable_normal(mut self) -> Self {
        self.normal = true;
        self
    }

    pub fn enable_color(mut self, default_color: Color) -> Self {
        self.color = true;
        self.colors.resize(24, default_color);
        self
    }

    pub fn set_color_x_y_z(mut self, color: Color) -> Self {
        self.colors[3] = color;
        self.colors[8] = color;
        self.colors[13] = color;
        self
    }

    pub fn set_color_nx_ny_nz(mut self, color: Color) -> Self {
        self.colors[7] = color;
        self.colors[18] = color;
        self.colors[20] = color;
        self
    }

    pub fn set_color_front_face(mut self, color: Color) -> Self {
        self.colors[0] = color;
        self.colors[1] = color;
        self.colors[2] = color;
        self.colors[3] = color;
        self
    }

    pub fn set_color_back_face(mut self, color: Color) -> Self {
        self.colors[20] = color;
        self.colors[21] = color;
        self.colors[22] = color;
        self.colors[23] = color;
        self
    }

    pub fn set_color_left_face(mut self, color: Color) -> Self {
        self.colors[16] = color;
        self.colors[17] = color;
        self.colors[18] = color;
        self.colors[19] = color;
        self
    }

    pub fn set_color_right_face(mut self, color: Color) -> Self {
        self.colors[8] = color;
        self.colors[9] = color;
        self.colors[10] = color;
        self.colors[11] = color;
        self
    }

    pub fn set_color_top_face(mut self, color: Color) -> Self {
        self.colors[12] = color;
        self.colors[13] = color;
        self.colors[14] = color;
        self.colors[15] = color;
        self
    }

    pub fn set_color_bottom_face(mut self, color: Color) -> Self {
        self.colors[4] = color;
        self.colors[5] = color;
        self.colors[6] = color;
        self.colors[7] = color;
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

        let a = 0.5f32;
        let na = -0.5f32;

        builder.add_position_vertices3(&[
            Vec3f::new(na, na, a),
            Vec3f::new(a, na, a),
            Vec3f::new(na, a, a),
            Vec3f::new(a, a, a),
            Vec3f::new(a, na, a),
            Vec3f::new(na, na, a),
            Vec3f::new(a, na, na),
            Vec3f::new(na, na, na),
            Vec3f::new(a, a, a),
            Vec3f::new(a, na, a),
            Vec3f::new(a, a, na),
            Vec3f::new(a, na, na),
            Vec3f::new(na, a, a),
            Vec3f::new(a, a, a),
            Vec3f::new(na, a, na),
            Vec3f::new(a, a, na),
            Vec3f::new(na, na, a),
            Vec3f::new(na, a, a),
            Vec3f::new(na, na, na),
            Vec3f::new(na, a, na),
            Vec3f::new(na, na, na),
            Vec3f::new(na, a, na),
            Vec3f::new(a, na, na),
            Vec3f::new(a, a, na),
        ]);
        builder.add_indices32(&[
            0, 1, 2, 3, 2, 1, 4, 5, 6, 7, 6, 5, 8, 9, 10, 11, 10, 9, 12, 13, 14, 15, 14, 13, 16,
            17, 18, 19, 18, 17, 20, 21, 22, 23, 22, 21,
        ]);
        if self.normal {
            let z = Vec3f::new(0f32, 0f32, 1f32);
            let nz = Vec3f::new(0f32, 0f32, -1f32);
            let x = Vec3f::new(1f32, 0f32, 0f32);
            let nx = Vec3f::new(-1f32, 0f32, 0f32);
            let y = Vec3f::new(0f32, 1f32, 0f32);
            let ny = Vec3f::new(0f32, -1f32, 0f32);

            builder.add_property_vertices(
                property,
                &[
                    z,
                    z, 
                    z, 
                    z,
                    ny,
                    ny,
                    ny,
                    ny,
                    x,
                    x,
                    x,
                    x,
                    y,
                    y,
                    y,
                    y,
                    nx,
                    nx,
                    nx,
                    nx,
                    nz,
                    nz,
                    nz,
                    nz
                ],
            );
        }
        if self.color {
            builder.add_property_vertices(color_property, &self.colors);
        }
        let mesh = builder.build().unwrap();
        mesh
    }
}
