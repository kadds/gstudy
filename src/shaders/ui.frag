#version 450 core
#extension GL_ARB_separate_shader_objects : enable

layout(binding = 0, set = 1) uniform texture2D texture2d;
layout(binding = 1, set = 0) uniform sampler sampler2d;

layout (location = 0) in vec2 tex_coord;
layout (location = 1) in vec4 color;

layout (location = 0) out vec4 out_color;

void main() {
    vec4 tex_c = texture(sampler2D(texture2d, sampler2d), tex_coord);
    out_color = tex_c.x * color;
}