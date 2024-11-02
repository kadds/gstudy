// includes
///#include "camera.wgsl"
///#include "./light.wgsl"
///#include "./material.wgsl"
///#if DIFFUSE_TEXTURE || NORMAL_TEXTURE || SPECULAR_TEXTURE || EMISSIVE_TEXTURE 
///#decl UV
///#endif

struct Object {
    model: mat4x4<f32>,
    inverse_model: mat4x4<f32>,
}

struct VertexInput {
    @loc_struct(VertexInput) position: vec3<f32>,
///#if NORMAL_VERTEX
    @loc_struct(VertexInput) normal: vec3<f32>,
///#endif
///#if DIFFUSE_VERTEX
    @loc_struct(VertexInput) diffuse: vec4<f32>,
///#endif
///#if SPECULAR_VERTEX
    @loc_struct(VertexInput) specular: vec4<f32>,
///#endif
///#if EMISSIVE_VERTEX
    @loc_struct(VertexInput) emissive: vec4<f32>,
///#endif
///#if UV
    @loc_struct(VertexInput) uv: vec2<f32>,
///#endif
}

struct VertexOutput {
///#if NORMAL_VERTEX
    @loc_struct(VertexOutput) normal: vec3<f32>,
///#endif
///#if DIFFUSE_VERTEX
    @loc_struct(VertexOutput) diffuse: vec3<f32>,
///#endif
///#if EMISSIVE_VERTEX
    @loc_struct(VertexOutput) emissive: vec3<f32>,
///#endif
///#if UV
    @loc_struct(VertexOutput) uv: vec2<f32>,
///#endif
///#if SHADOW
    @loc_struct(VertexOutput) shadow_position: vec3<f32>,
///#endif
    @loc_struct(VertexOutput) @builtin(position) position: vec4<f32>,
};

struct BaseLightUniform {
    ambient: vec3<f32>,
    placement: f32,
///#if DIRECT_LIGHT
    direct: DirectLight
///#endif
}

///#if MATERIAL
struct MaterialUniform {
///#if CONST_COLOR
    color: vec3<f32>,
///#endif
///#if ALPHA_TEST
    alpha_test: f32,
///#endif
}
///#endif

@loc_global(CameraUniform) var<uniform> camera_uniform: CameraUniform;
@loc_global(LightUniform) var<uniform> light_uniform: BaseLightUniform;

///#if MATERIAL
@loc_global(MaterialUniform) var<uniform> material_uniform: MaterialUniform;
///#endif
///#if UV
@loc_global(MaterialUniform) var sampler_tex: sampler;
///#endif
///#if DIFFUSE_TEXTURE
@loc_global(MaterialUniform) var texture_diffuse: texture_2d<f32>;
///#endif

///#if NORMAL_TEXTURE
@loc_global(MaterialUniform) var texture_normal: texture_2d<f32>;
///#endif

///#if SPECULAR_TEXTURE
@loc_global(MaterialUniform) var texture_specular: texture_2d<f32>;
///#endif

///#if EMISSIVE_TEXTURE
@loc_global(MaterialUniform) var texture_emissive: texture_2d<f32>;
///#endif

///#if SHADOW
@loc_global(ShadowUniform) var shadow_sampler: sampler_comparison;
@loc_global(ShadowUniform) var shadow_map: texture_depth_2d;
///#endif

@loc_global(ObjectUniform) var<push_constant> object: Object;

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
///#if EMISSIVE_VERTEX
    output.emissive = input.emissive.xyz;
///#endif
///#if UV
    output.uv = input.uv;
///#endif
///#if SHADOW
    let pos_camera = light_uniform.direct.vp * (object.model * vec4<f32>(input.position, 1.0));
    let pos_camera_norm = pos_camera.xyz / pos_camera.w;
    let p = pos_camera_norm.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    output.shadow_position = vec3<f32>(p.x, p.y, pos_camera_norm.z);
///#endif

    return output;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32>{
    var obj: ObjectInfo;
///#if DIFFUSE_CONSTANT

    obj.color = material_uniform.diffuse;
///#if DIFFUSE_VERTEX
    obj.color *= input.diffuse;
///#endif
///#if DIFFUSE_TEXTURE
    obj.color *= textureSample(texture_diffuse, sampler_tex, input.uv).xyz;
///#endif

///#else

    obj.color = vec3<f32>(1.0, 1.0, 1.0);
///#if DIFFUSE_VERTEX
    obj.color *= input.diffuse;
///#endif
///#if DIFFUSE_TEXTURE
    obj.color = textureSample(texture_diffuse, sampler_tex, input.uv).xyz;
///#endif
///#if DIFFUSE_TEXTURE && DIFFUSE_VERTEX
///#else
    obj.color = vec3<f32>(0.0, 0.0, 0.0);
///#endif

///#endif


///#if NORMAL_VERTEX
    obj.normal = transform_normal_worldspace(input.normal, object.inverse_model);
///#elseif NORMAL_TEXTURE
    obj.normal = transform_normal_worldspace(textureSample(texture_normal, sampler_tex, input.uv).xyz, 
        object.inverse_model);
///#else
    obj.normal = vec3<f32>(0.0, 1.0, 0.0);
///#endif

    var ambient_color = ambient(obj, light_uniform.ambient);
    var color = vec3<f32>(0.0, 0.0, 0.0);

///#if DIRECT_LIGHT
    let intensity = light_uniform.direct.intensity;
    var light: LightInfo;
    light.dir = light_uniform.direct.direction;
    light.color = light_uniform.direct.color;
    color += diffuse(obj, light) * intensity;
///#if SPECULAR_VERTEX
    obj.color = input.specular;
///#elseif SPECULAR_CONSTANT
    obj.color = material_uniform.specular;
///#elseif SPECULAR_TEXTURE
    obj.color = textureSample(texture_specular, sampler_tex, input.uv).xyz;
///#else
    obj.color = vec3<f32>(0.0, 0.0, 0.0);
///#endif
    color += specular(obj, light, camera_uniform.dir, material_uniform.shininess);
///#endif

///#if EMISSIVE
    var emissive_color = material_uniform.emissive;
///#if EMISSIVE_VERTEX
    emissive_color *= input.emissive;
///#elseif EMISSIVE_TEXTURE
    emissive_color *= textureSample(texture_emissive, sampler_tex, input.uv).xyz;
///#endif
    color += emissive_color * material_uniform.emissive_strength;
///#endif

///#if SHADOW
    let size = vec2<f32>(light_uniform.direct.size_x, light_uniform.direct.size_y);
    let shadow = recv_shadow_visibility(input.shadow_position, 
        obj.normal, light.dir,
        shadow_sampler, shadow_map, size, light_uniform.direct.bias_factor);
    color = color * shadow + ambient_color;
///#else
    color = color + ambient_color;
///#endif

    return vec4<f32>(color.xyz, 1.0);
}
