// includes
///#include "camera.wgsl"
///#include "./light.wgsl"
///#include "./material.wgsl"
///#if DIFFUSE_TEXTURE || NORMAL_TEXTURE || SPECULAR_TEXTURE || EMISSIVE_TEXTURE 
///#decl UV
///#endif

///#decl POSITION_VERTEX_INPUT = _atomic_counter(0, 1)
///#decl POSITION_VERTEX_OUTPUT = _atomic_counter(0, 1)
///#decl BINDING_GLOBAL_GROUP1 = _atomic_counter(1, 1)

struct Object {
    model: mat4x4<f32>,
    inverse_model: mat4x4<f32>,
}

struct VertexInput {
    @location(#{POSITION_VERTEX_INPUT}) position: vec3<f32>,
///#if NORMAL_VERTEX
    @location(#{POSITION_VERTEX_INPUT}) normal: vec3<f32>,
///#endif
///#if DIFFUSE_VERTEX
    @location(#{POSITION_VERTEX_INPUT}) diffuse: vec4<f32>,
///#endif
///#if SPECULAR_VERTEX
    @location(#{POSITION_VERTEX_INPUT}) specular: vec4<f32>,
///#endif
///#if UV
    @location(#{POSITION_VERTEX_INPUT}) uv: vec2<f32>,
///#endif
}

struct VertexOutput {
///#if NORMAL_VERTEX
    @location(#{POSITION_VERTEX_OUTPUT}) normal: vec3<f32>,
///#endif
///#if DIFFUSE_VERTEX
    @location(#{POSITION_VERTEX_OUTPUT}) diffuse: vec3<f32>,
///#endif
///#if UV
    @location(#{POSITION_VERTEX_OUTPUT}) uv: vec2<f32>,
///#endif
    @builtin(position) position: vec4<f32>,
};

struct AddLightUniform {
///#if POINT_LIGHT
    point: PointLight,
///#elseif SPOT_LIGHT
    spot: SpotLight,
///#endif
}

@group(0) @binding(0) var<uniform> camera_uniform: CameraUniform;
@group(1) @binding(0) var<uniform> light_uniform: AddLightUniform;

@group(2) @binding(0) var<uniform> material_uniform: MaterialUniform;

///#if UV
@group(2) @binding(#{BINDING_GLOBAL_GROUP1}) var sampler_tex: sampler;
///#endif

///#if DIFFUSE_TEXTURE
@group(2) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_color: texture_2d<f32>;
///#endif

///#if NORMAL_TEXTURE
@group(2) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_normal: texture_2d<f32>;
///#endif

///#if SPECULAR_TEXTURE
@group(2) @binding(#{BINDING_GLOBAL_GROUP1}) var texture_specular: texture_2d<f32>;
///#endif

@group(3) @binding(0) var sampler_shadow_map: sampler;
@group(3) @binding(1) var texture_shadow_map: texture_depth_2d;

var<push_constant> object: Object;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput{
    var output: VertexOutput;
    output.position = camera_uniform.vp * (object.model * vec4<f32>(input.position, 1.0));
///#if NORMAL_VERTEX
    output.normal = input.normal;
///#endif
///#if DIFFUSE_VERTEX
    output.diffuse = input.diffuse.xyz;
///#endif
///#if UV
    output.uv = input.uv;
///#endif

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32>{
    var obj: ObjectInfo;
///#if DIFFUSE_VERTEX
    obj.color = input.diffuse;
///#elseif DIFFUSE_CONSTANT
    obj.color = material_uniform.diffuse;
///#else
    obj.color = vec3<f32>(0.0, 0.0, 0.0);
///#endif

///#if NORMAL_VERTEX
    obj.normal = transform_normal_worldspace(input.normal, object.inverse_model);
///#endif

    var color = vec3<f32>(0.0, 0.0, 0.0);

    var light: LightInfo;
///#if POINT_LIGHT
    light.dir = input.position.xyz - light_uniform.point.position;
    light.color = light_uniform.point.color;
    color += diffuse(obj, light);
///#endif

///#if SPOT_LIGHT
    light.dir = light_uniform.spot.direction;
    light.color = light_uniform.spot.color;
    color += diffuse(obj, light);
///#endif

///#if SPECULAR_VERTEX
    obj.color = input.specular;
///#elseif SPECULAR_CONSTANT
    obj.color = material_uniform.specular;
///#else
    obj.color = vec3<f32>(0.0, 0.0, 0.0);
///#endif

    color += specular(obj, light, camera_uniform.dir, material_uniform.shininess);

///#if EMISSIVE_VERTEX
    color += input.emissive;
///#elseif EMISSIVE_CONSTANT
    color += material_uniform.emissive;
///#endif

    let shadow = 1.0;
    color = color * shadow;

    return vec4<f32>(color.xyz, 1.0);
}
