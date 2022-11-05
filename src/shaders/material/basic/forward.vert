/// compile flags
/// :
/// c: VERTEX_COLOR
/// t: VERTEX_TEX

#version 450 core
layout (location = 0) in vec3 pos;
#ifdef VERTEX_COLOR
layout (location = 1) in vec4 color;
#endif

#ifdef VERTEX_TEX
layout (location = 2) in vec2 tex;
#endif

layout (binding = 0, set = 0) uniform local { mat4x4 mvp; };

#ifdef VERTEX_COLOR
layout (location = 0) out vec4 o_color;
#endif

#ifdef VERTEX_TEX
layout (location = 1) out vec2 o_tex;
#endif

void main() {
    gl_Position = mvp * vec4(pos, 1.0);
    #ifdef VERTEX_COLOR
    o_color = color;
    #endif
    #ifdef VERTEX_TEX
    o_tex = tex;
    #endif
}
