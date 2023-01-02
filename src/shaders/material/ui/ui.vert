#version 450 core
layout (location = 0) in vec2 pos;
layout (location = 1) in vec2 tex;
layout (location = 2) in uint color;

layout (set = 0, binding = 0) uniform local { vec2 screen_size; };

layout (location = 0) out vec2 o_tex;
layout (location = 1) out vec4 o_color;

vec3 srgb_2_linear(vec3 srgb) {
    bvec3 cutoff = lessThan(srgb, vec3(10.31475));
    vec3 lower = srgb / vec3(3294.6);
    vec3 higher = pow((srgb + vec3(14.025)) / vec3(269.025), vec3(2.4));
    return mix(higher, lower, cutoff);
}

void main() {
    gl_Position = vec4(2.0 * pos.x / screen_size.x - 1.0, 1.0 - 2.0 * pos.y / screen_size.y, 0.0, 1.0);
    o_tex = tex;
    // srgba -> linear color
    vec4 color = vec4(color & 0xFFu, (color >> 8) & 0xFFu, (color >> 16) & 0xFFu, (color >> 24) & 0xFFu);
    // o_color = vec4(srgb_2_linear(color.rgb), color.a / 255.0);
    o_color = vec4(color.r / 255.0, color.g / 255.0, color.b / 255.0, color.a / 255.0);
}
