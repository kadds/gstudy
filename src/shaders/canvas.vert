#version 450 core
layout (location = 0) in vec2 pos;
layout (location = 1) in vec2 tex;

layout (binding = 0, set = 0) uniform local { vec2 screen_size; };

layout (location = 0) out vec2 o_tex;

void main() {
    gl_Position = vec4(2.0 * pos.x / screen_size.x - 1.0, 1.0 - 2.0 * pos.y / screen_size.y, 0.0, 1.0);
    o_tex = tex;
}
