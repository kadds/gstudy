// includes
///#include "camera.wgsl"
///#include "object.wgsl"
///#if TEXTURE
///#decl VERTEX_TEX
///#endif

///#decl POSITION_VERTEX_INPUT = _atomic_counter(0, 1)
///#decl POSITION_VERTEX_OUTPUT = _atomic_counter(0, 1)
///#decl BINDING_GLOBAL_GROUP1 = _atomic_counter(0, 1)

struct VertexInput {
    @location(#{POSITION_VERTEX_INPUT}) position: vec3<f32>,
///#if VERTEX_COLOR
    @location(#{POSITION_VERTEX_INPUT}) color: vec4<f32>,
///#endif
///#if VERTEX_TEX
    @location(#{POSITION_VERTEX_INPUT}) uv: vec2<f32>,
///#endif
}

struct VertexOutput {
    @location(#{POSITION_VERTEX_OUTPUT}) color: vec4<f32>,
///#if VERTEX_TEX
    @location(#{POSITION_VERTEX_OUTPUT}) uv: vec2<f32>,
///#endif
    @builtin(position) position: vec4<f32>,
};
///#if CONST_COLOR || ALPHA_TEST
///#decl MATERIAL
///#endif

///#if MATERIAL
struct MaterialUniform {
///#if CONST_COLOR
    color: vec4<f32>,
///#endif
///#if ALPHA_TEST
    alpha_test: f32,
///#endif
}
///#endif

@group(0) @binding(0) var<uniform> camera_uniform: CameraUniform;
///#if MATERIAL
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var<uniform> material_uniform: MaterialUniform;
///#endif

///#if VERTEX_TEX
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var sampler_tex: sampler;
///#endif

///#if TEXTURE
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_color: texture_2d<f32>;
///#endif

var<push_constant> object: Object;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput{
    var output: VertexOutput;
    output.position = camera_uniform.vp * object.model * vec4<f32>(input.position, 1.0);
///#if CONST_COLOR
    output.color = material_uniform.color;
///#else
    output.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
///#endif

///#if VERTEX_COLOR
    output.color *= input.color;
///#endif

///#if VERTEX_TEX
    output.uv = input.uv;
///#endif

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32>{
    var color = input.color;
///#if TEXTURE
    color *= textureSample(texture_color, sampler_tex, input.uv);
///#endif

///#if ALPHA_TEST
    if (color.a < material_uniform.alpha_test) {
        discard;
    }
///#endif
    return color;
}
