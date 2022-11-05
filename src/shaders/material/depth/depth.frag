#version 450 core
#extension GL_ARB_separate_shader_objects : enable

layout (location = 0) out vec4 out_color;

void main() {
    out_color = vec4(0.0, 0.0, gl_FragCoord.z / gl_FragCoord.w, 1.0);
}