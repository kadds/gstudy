///#include "camera.wgsl"

struct VertexInput {
    @loc_struct(VertexInput) position: vec3<f32>,
}

struct VertexOutput {
    @loc_struct(VertexOutput) @builtin(position) position: vec4<f32>,
};

struct Object {
    model: mat4x4<f32>,
}

struct ShadowUniform {
    vp: mat4x4<f32>,
    dir: vec3<f32>,
    placement: f32,
    znear: f32,
    zfar: f32,
    need_transform_to_linear: f32,
}

@loc_global(ShadowUniform) var<uniform> shadow_uniform: ShadowUniform;

@loc_global(ObjectUniform) var<push_constant> object: Object;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput{
    var output: VertexOutput;
    output.position = shadow_uniform.vp * (object.model * vec4<f32>(input.position, 1.0));

    return output;
}

struct FragmentOutput {
   @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    var output: FragmentOutput;
    // if shadow_uniform.need_transform_to_linear > 0.0 {
    //     output.depth = (input.position.z - shadow_uniform.znear) / (shadow_uniform.zfar - shadow_uniform.znear);
    // } else {
        output.depth = input.position.z;
    // }
    return output;
}