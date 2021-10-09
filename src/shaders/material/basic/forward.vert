#version 450 core
layout (location = 0) in vec3 pos;
// layout (location = 1) in vec2 tex;

layout (binding = 0, set = 0) uniform local { mat4x4 mvp; };

//layout (location = 0) out vec2 o_tex;

void main() {
    gl_Position = mvp * vec4(pos, 1.0);
    // o_tex = tex;
}
