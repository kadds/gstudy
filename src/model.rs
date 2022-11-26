use crate::geometry::*;

#[derive(Debug)]
pub struct Model {
    path: String,
    mesh: Mesh,
}

impl Model {
    pub fn new(path: String, mesh: Mesh) -> Self {
        Self { path, mesh }
    }
    // pub fn object() -> Object {

    // }
}
