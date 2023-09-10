
struct MaterialUniform {
///#if DIFFUSE_CONSTANT
    diffuse: vec3<f32>,
    placement1: f32,
///#endif
///#if SPECULAR_CONSTANT
    specular: vec3<f32>,
    placement2: f32,
///#endif
    emissive: vec3<f32>,
    emissive_strength: f32,

    shininess: f32,
///#if ALPHA_TEST
    alpha_test: f32,
///#endif
}
