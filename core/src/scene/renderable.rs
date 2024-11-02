use std::sync::Arc;

use crate::mesh::Geometry;


pub struct Renderable {
    geometry: Arc<dyn Geometry>,
}

