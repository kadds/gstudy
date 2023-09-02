struct LightInfo {
    dir: vec3<f32>,
    color: vec3<f32>,
}

struct ObjectInfo {
    color: vec3<f32>,
    normal: vec3<f32>,
}

fn ambient(object: ObjectInfo, ambient: vec3<f32>) -> vec3<f32> {
    return object.color * ambient;
}

fn diffuse(object: ObjectInfo, light: LightInfo) -> vec3<f32> {
    let diffuse_factor = max(dot(-light.dir, object.normal), 0.0);
    let diffuse_color = light.color * object.color;
    return diffuse_factor * diffuse_color;
}

fn specular(object: ObjectInfo, light: LightInfo, view: vec3<f32>, shininess: f32) -> vec3<f32> {
    let h = normalize(-view - light.dir);
    let factor = pow(max(dot(h, object.normal), 0.0), shininess);
    let color = light.color * object.color;

    return factor * color;
}

struct DirectLight {
    color: vec3<f32>,
    placement: f32,
    direction: vec3<f32>,
    placement2: f32,
}

fn transform_normal_worldspace(normal: vec3<f32>, world_inv: mat4x4<f32>) -> vec3<f32> {
    return normalize(normal * mat3x3<f32>(world_inv[0].xyz, world_inv[1].xyz, world_inv[2].xyz));
}

fn recv_shadow(position: vec4<f32>, sampler_tex: sampler, shadow_tex: texture_depth_2d) -> f32 {
    let uv = vec2<f32>(0.0, 0.0);
    let color = textureSample(shadow_tex, sampler_tex, uv);
    return color;
}


struct PointLight {
    color: vec3<f32>,
    placement: f32,
    position: vec3<f32>,
    placement2: f32,
}

struct SpotLight {
    color: vec3<f32>,
    placement: f32,
    position: vec3<f32>,
    cutoff: f32,
    direction: vec3<f32>,
    cutoff_outer: f32,
}
