
struct CameraUniform {
    vp: mat4x4<f32>,
    dir: vec3<f32>,
    placement: f32,
};

struct D2SizeCameraUniform {
    view_size: vec4<f32>,
}