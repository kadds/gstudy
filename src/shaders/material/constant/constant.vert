#version 450 core
layout (location = 0) in vec3 pos;

layout (binding = 0, set = 0) uniform local { mat4x4 mvp; };

void main() {
    gl_Position = mvp * vec4(pos, 1.0);
}
