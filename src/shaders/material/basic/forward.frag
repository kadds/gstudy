/// compile flags
/// :
/// c: VERTEX_COLOR
/// t: VERTEX_TEX
/// ct: VERTEX_COLOR, VERTEX_TEX

#version 450 core
#extension GL_ARB_separate_shader_objects : enable
layout (set = 1, binding = 0) uniform const_parameter_material { vec4 const_color; }; // per material update

#ifdef VERTEX_TEX
layout(set = 1, binding = 1) uniform sampler sampler2d; // per material update
layout(set = 1, binding = 2) uniform texture2D texture2d;  // per material update
#endif

#ifdef VERTEX_COLOR
layout (location = 0) in vec4 vertex_color;
#endif


#ifdef VERTEX_TEX
layout (location = 1) in vec2 vertex_tex;
#endif

layout (location = 0) out vec4 out_color;

void main() {
    out_color = const_color;

    #ifdef VERTEX_COLOR
    out_color *= vertex_color;
    #endif

    #ifdef VERTEX_TEX
    vec4 tex_c = texture(sampler2D(texture2d, sampler2d), vertex_tex);
    out_color *= tex_c;
    #endif

}