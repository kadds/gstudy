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

fn transform_normal_worldspace(normal: vec3<f32>, world_inv: mat4x4<f32>) -> vec3<f32> {
    return normalize(normal * mat3x3<f32>(world_inv[0].xyz, world_inv[1].xyz, world_inv[2].xyz));
}

///#if SHADOW_PCF
fn recv_shadow_visibility(pos: vec3<f32>, normal: vec3<f32>, light_dir: vec3<f32>, 
    sampler_tex: sampler_comparison, shadow_tex: texture_depth_2d, size: vec2<f32>) -> f32 {
    let ov = 1.0 / size;

    // let bias = max(0.01 * (1.0 - dot(normal, -light_dir)), 0.005);
    var visibility = 0.0;
    let bias = 0.0;

    for (var x = -1; x <= 1; x++) {
        for (var y = -1; y <= 1; y++) {
            visibility += textureSampleCompare(shadow_tex, sampler_tex, pos.xy + vec2<f32>(vec2(x, y)) * ov, pos.z - bias);
        }
    }

    if pos.z > 1.0 {
        return 1.0;
    }

    return visibility / 9.0;
}
///#else
fn recv_shadow_visibility(pos: vec3<f32>, normal: vec3<f32>, light_dir: vec3<f32>, 
    sampler_tex: sampler_comparison, shadow_tex: texture_depth_2d, size: vec2<f32>) -> f32 {

    // let bias = max(0.05 * (1.0 - dot(normal, -light_dir)), 0.005);
    let bias = 0.0;

    var visibility = textureSampleCompare(shadow_tex, sampler_tex, pos.xy, pos.z - bias);

    if pos.z >= 1.0 {
        return 1.0;
    }

    return visibility;
}
///#endif

fn get_attenuation(distance: f32, at: vec4<f32>) -> f32 {
    if at.w < distance {
        return 0.0;
    }
    let attenuation = at.x + at.y * distance + at.z * distance * distance;
    return 2.0 / (attenuation + 1.0);
}

struct DirectLight {
    color: vec3<f32>,
    size_x: f32,
    direction: vec3<f32>,
    size_y: f32,
    vp: mat4x4<f32>,
    attenuation: vec4<f32>,
    intensity: vec4<f32>,
}

struct PointLight {
    color: vec3<f32>,
    size_x: f32,
    position: vec3<f32>,
    size_y: f32,
    vp: mat4x4<f32>,
    attenuation: vec4<f32>,
    intensity: vec4<f32>,
}

struct SpotLight {
    vp: mat4x4<f32>,
    color: vec3<f32>,
    size_x: f32,
    position: vec3<f32>,
    size_y: f32,
    direction: vec3<f32>,
    placement: f32,
    cutoff: f32,
    cutoff_outer: f32,
    placement2: f32,
    intensity: f32,
    attenuation: vec4<f32>,
}
