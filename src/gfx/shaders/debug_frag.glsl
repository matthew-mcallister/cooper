#version 450
#pragma shader_stage(fragment)

#include "common_inc.glsl"

layout(constant_id = CONST_ID_DEBUG_DISPLAY)
const uint DEBUG_DISPLAY = DEBUG_DISPLAY_DEPTH;

layout(location = 0) in vec3 in_normal;

layout(location = 0) out vec4 out_color;

void main() {
    if (DEBUG_DISPLAY != DEBUG_DISPLAY_DEPTH) {
        out_color = vec4(vec3(gl_FragCoord.z / gl_FragCoord.w), 1.0);
    } else {
        out_color = vec4(-0.5 * in_normal + vec3(0.5), 1.0);
    }
}
