///#include "./camera.wgsl"

struct VertexInput {
    @location(0) position: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

struct Object {
    model: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera_uniform: CameraUniform;

var<push_constant> object: Object;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput{
    var output: VertexOutput;
    output.position = camera_uniform.vp * object.model * input.position;

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32>{
    var color = vec4(0, 0, (input.position.z / input.position.w) * 0.8 + 0.2, 1.0);

    return color;
}
