// includes
///#include "camera.wgsl"
///#include "./light.wgsl"
///#include "./material.wgsl"
///#if DIFFUSE_TEXTURE || NORMAL_TEXTURE || SPECULAR_TEXTURE
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
///#if UV
    @loc_struct(VertexOutput) uv: vec2<f32>,
///#endif
    @loc_struct(VertexOutput) raw_position: vec3<f32>,
///#if SHADOW
    @loc_struct(VertexOutput) shadow_position: vec3<f32>,
///#endif
    @loc_struct(VertexOutput)  @builtin(position) position: vec4<f32>,
};

struct AddLightUniform {
///#if POINT_LIGHT
    point: PointLight,
///#elseif SPOT_LIGHT
    spot: SpotLight,
///#endif
}

struct MaterialUniform {

}

@loc_global(CameraUniform) var<uniform> camera_uniform: CameraUniform;
@loc_global(LightUniform) var<uniform> light_uniform: AddLightUniform;

@loc_global(MaterialUniform) var<uniform> material_uniform: MaterialUniform;

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
///#if UV
    output.uv = input.uv;
///#endif
///#if POINT_LIGHT
    let pos_camera = light_uniform.point.vp * (object.model * vec4<f32>(input.position, 1.0));
    let pos_camera_norm = pos_camera.xyz / pos_camera.w;
    // let z = (pos_camera_norm.z - light_uniform.point.near) / (light_uniform.point.far - light_uniform.point.near);
///#elseif SPOT_LIGHT
    let pos_camera = light_uniform.spot.vp * (object.model * vec4<f32>(input.position, 1.0));
    let pos_camera_norm = pos_camera.xyz / pos_camera.w;
    // let z = (pos_camera_norm.z - light_uniform.spot.near) / (light_uniform.spot.far - light_uniform.spot.near);
///#endif
///#if SHADOW
    let p = pos_camera_norm.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    output.shadow_position = vec3<f32>(p.x, p.y, pos_camera_norm.z);
///#endif
    output.raw_position = (object.model * vec4<f32>(input.position, 1.0)).xyz;

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
    obj.color *= textureSample(texture_diffuse, sampler_tex, input.uv).xyz;
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

    var color = vec3<f32>(0.0, 0.0, 0.0);

    var light: LightInfo;
    var intensity = 1.0;
    var attenuation: vec4<f32>;
///#if POINT_LIGHT
    let distance = length(input.raw_position - light_uniform.point.position);
    light.dir = normalize(input.raw_position - light_uniform.point.position);
    light.color = light_uniform.point.color;
    intensity = light_uniform.point.intensity;
    attenuation = light_uniform.point.attenuation;
///#elseif SPOT_LIGHT
    let distance = length(input.raw_position - light_uniform.spot.position);
    light.dir = normalize(input.raw_position - light_uniform.spot.position);
    light.color = light_uniform.spot.color;
    intensity = light_uniform.spot.intensity;
    attenuation = light_uniform.spot.attenuation;
///#endif
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

    color += specular(obj, light, camera_uniform.dir, material_uniform.shininess) * intensity;

    var value = get_attenuation(distance, attenuation);
///#if POINT_LIGHT
///#if SHADOW
    let size = vec2<f32>(light_uniform.point.size_x, light_uniform.point.size_y);
///#endif
    let bias_factor = light_uniform.point.bias_factor;

///#elseif SPOT_LIGHT

    let theta = acos(dot(light.dir, light_uniform.spot.direction));
    if theta > light_uniform.spot.cutoff_outer {
        value = 0.0; // shadow
    } else {
        if theta > light_uniform.spot.cutoff {
            let total = light_uniform.spot.cutoff_outer - light_uniform.spot.cutoff;
            value = 1.0 - (theta - light_uniform.spot.cutoff) / total; // 
        } else {
            value = 1.0;
        }
    }
    let bias_factor = light_uniform.spot.bias_factor;

///#if SHADOW
    if value > 0.0 {
        let size = vec2<f32>(light_uniform.spot.size_x, light_uniform.spot.size_y);
        let shadow = recv_shadow_visibility(input.shadow_position, 
            obj.normal, light.dir,
            shadow_sampler, shadow_map, size, bias_factor);
        value *= shadow;
    }
///#endif

///#endif
    color = color * value;
    

    return vec4<f32>(color.xyz, 1.0);
}
