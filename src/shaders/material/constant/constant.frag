#version 450 core
#extension GL_ARB_separate_shader_objects : enable

layout (location = 0) out vec4 out_color;
layout (binding = 1, set = 0) uniform local { vec4 c_color; };

void main() {
    out_color = c_color;
}