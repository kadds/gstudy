use core::types::Color;

pub struct DirectLight {
    color: Color,
}

pub struct PointLight {
    color: Color,
}

pub struct SpotLight {
    color: Color,
}

pub struct SceneLights {
    direct_light: Option<DirectLight>,
    point_light: Vec<PointLight>,
    spot_light: Vec<SpotLight>,
}

impl SceneLights {}
