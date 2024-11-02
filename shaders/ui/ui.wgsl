///#include "./camera.wgsl"

struct VertexOutput {
    @loc_struct(VertexOutput) uv: vec2<f32>,
    @loc_struct(VertexOutput) color: vec4<f32>,
    @loc_struct(VertexOutput) @builtin(position) position: vec4<f32>,
};

@loc_global(CameraUniform) var<uniform> camera_uniform: D2SizeCameraUniform;
@loc_global(MaterialUniform) var texture: texture_2d<f32>;
@loc_global(MaterialUniform) var texture_sampler: sampler;

struct VertexInput {
    @loc_struct(VertexInput) pos: vec2<f32>,
    @loc_struct(VertexInput) uv: vec2<f32>,
    @loc_struct(VertexInput) color: u32,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(2.0 * input.pos.x / camera_uniform.view_size.x - 1.0,
        1.0 - 2.0 * input.pos.y / camera_uniform.view_size.y, 0.0, 1.0);
    out.uv = input.uv;
    out.color = vec4<f32>(f32(input.color & 0xFFu) / 255.0, f32((input.color >> 8u) & 0xFFu) / 255.0,
        f32((input.color >> 16u) & 0xFFu) / 255.0, f32((input.color >> 24u) & 0xFFu) / 255.0);

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(texture, texture_sampler, input.uv);
    return tex * input.color;
}
