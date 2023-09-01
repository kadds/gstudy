
struct Object {
    model: mat4x4<f32>,
///#if INVERSE_OBJECT
    inverse_model: mat3x3<f32>,
    placement: array<f32, 4>,
///#endif
}
