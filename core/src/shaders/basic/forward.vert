/// compile flags
/// :
/// c: VERTEX_COLOR
/// t: VERTEX_TEX
/// a: ALPHA_TET
/// ct: VERTEX_COLOR, VERTEX_TEX
/// ca: VERTEX_COLOR, ALPHA_TEST
/// ta: VERTEX_TEX, ALPHA_TEST
/// cta: VERTEX_TEX, ALPHA_TEST, VERTEX_COLOR

#version 450 core
layout (set = 0, binding = 0) uniform local { mat4x4 vp; }; // per camera update
layout (set = 1, binding = 0) uniform const_parameter_material {
    vec4 const_color;
#ifdef ALPHA_TEST
    float alpha_test_val;
#endif
}; // per material update
layout (set = 2, binding = 0) uniform per_object { mat4x4 model; }; // per object update

layout (location = 0) in vec3 pos;
#ifdef VERTEX_COLOR
layout (location = 1) in vec4 color;
#endif

#ifdef VERTEX_TEX
layout (location = 2) in vec2 tex;
#endif

#ifdef VERTEX_COLOR
layout (location = 0) out vec4 o_color;
#endif

#ifdef VERTEX_TEX
layout (location = 1) out vec2 o_tex;
#endif

void main() {
    gl_Position = vp * model * vec4(pos, 1.0);
    #ifdef VERTEX_COLOR
    o_color = color;
    #endif
    #ifdef VERTEX_TEX
    o_tex = tex;
    #endif
}
