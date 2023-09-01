// includes
///#include "./camera.wgsl"
///#if TEXTURE_COLOR || NORMAL_TEX || HEIGHT_TEX || EMISSION_TEX
///#decl VERTEX_TEX
///#endif

///#decl POSITION_VERTEX_INPUT = _atomic_counter(0, 1)
///#decl POSITION_VERTEX_OUTPUT = _atomic_counter(0, 1)
///#decl BINDING_GLOBAL_GROUP1 = _atomic_counter(1, 1)

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

struct MaterialUniform {
    ambient: vec4<f32>,
    diffuse: vec3<f32>,
    specular: vec3<f32>,
    shininess: f32,
///#if ALPHA_TEST
    alpha_test: f32,
///#endif
}

struct DirectLight {
    color: vec3<f32>,
    placement: f32,
    direction: vec3<f32>,
}

struct PointLight {
    color: vec3<f32>,
    attenuation_linear: f32,
    attenuation_quadratic: f32,
    attenuation_constant: f32,
}

struct SpotLight {
    color: vec3<f32>,
    attenuation_linear: f32,
    direction: vec3<f32>,
    attenuation_quadratic: f32,
    attenuation_constant: f32,
    cutoff_outer: f32,
    cutoff: f32,
}

struct LightUniform {
    direct: DirectLight
}

struct Object {
    model: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera_uniform: CameraUniform;
@group(1) @binding(0) var<uniform> material_uniform: MaterialUniform;

///#if VERTEX_TEX
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var sampler_tex: sampler;
///#endif

///#if TEXTURE_COLOR
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_color: texture_2d<f32>;
///#endif

///#if NORMAL_TEX
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_normal: texture_2d<f32>;
///#endif

///#if HEIGHT_TEX
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_height: texture_2d<f32>;
///#endif

///#if EMISSION_TEX
@group(1) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_emission: texture_2d<f32>;
///#endif

var<push_constant> object: Object;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput{
    var output: VertexOutput;
    output.position = camera_uniform.vp * object.model * vec4<f32>(input.position, 1.0);
    output.color = vec4<f32>(1.0,1.0,1.0,1.0);
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
///#if TEXTURE_COLOR
    color *= textureSample(texture_color, sampler_tex, input.uv);
///#endif
///#if EMISSION_TEX
    color *= textureSample(texture_emission, sampler_tex, input.uv);
///#endif

///#if ALPHA_TEST
    if (color.a < material_uniform.alpha_test) {
        discard;
    }
///#endif
    return color;
}
