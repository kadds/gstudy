/// compile flags
/// :
/// c: VERTEX_COLOR
/// t: VERTEX_TEX

#version 450 core
#extension GL_ARB_separate_shader_objects : enable

#ifdef VERTEX_TEX
layout(binding = 0, set = 1) uniform texture2D texture2d;
layout(binding = 1, set = 0) uniform sampler sampler2d;
#endif

#ifdef VERTEX_COLOR
layout (location = 0) in vec4 vertex_color;
#endif

#ifdef VERTEX_TEX
layout (location = 1) in vec2 vertex_tex;
#endif

layout (location = 0) out vec4 out_color;

void main() {
    out_color = vec4(1.0, 1.0, 1.0, 1.0);

    #ifdef VERTEX_COLOR
    out_color = vertex_color;
    #endif

    #ifdef VERTEX_TEX
    vec4 tex_c = texture(sampler2D(texture2d, sampler2d), vertex_tex);
    out_color = tex_c;
    #endif

}