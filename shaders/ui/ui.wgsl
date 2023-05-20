///#include "./camera.wgsl"

struct VertexOutput {
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@group(0) @binding(0) var<uniform> camera_uniform: D2SizeCameraUniform;
@group(1) @binding(0) var texture: texture_2d<f32>;
@group(1) @binding(1) var texture_sampler: sampler;

@vertex
fn vs_main(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: u32) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(2.0 * pos.x / camera_uniform.view_size.x - 1.0,
        1.0 - 2.0 * pos.y / camera_uniform.view_size.y, 0.0, 1.0);
    out.uv = uv;
    out.color = vec4<f32>(f32(color & 0xFFu) / 255.0, f32((color >> 8u) & 0xFFu) / 255.0,
        f32((color >> 16u) & 0xFFu) / 255.0, f32((color >> 24u) & 0xFFu) / 255.0);

    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(texture, texture_sampler, input.uv);
    return tex * input.color;
}
