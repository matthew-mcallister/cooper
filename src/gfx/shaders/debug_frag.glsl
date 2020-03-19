#version 450
#pragma shader_stage(fragment)

#include "common_inc.glsl"

// TODO: Consider declaring all constants in header
layout(constant_id = CONST_ID_DEBUG_DISPLAY)
const uint DEBUG_DISPLAY = DEBUG_DISPLAY_DEPTH;

layout(set = 0, binding = 0) uniform blk_scene {
    SceneView g_scene_view;
};

layout(location = 0) in vec3 in_normal;

layout(location = 0) out vec4 out_color;

void main() {
    if (DEBUG_DISPLAY == DEBUG_DISPLAY_DEPTH) {
        // TODO: configurable saturation point for interior scenes
        float z = 1 / gl_FragCoord.w;
        float z_far = g_scene_view.perspective.z_far;
        // Invert to mimic the real Z buffer
        float value = 1.0 - z / z_far;
        // Seems to improve the black curve; might depend on your screen
        for (uint i = 0; i < 5; i++) {
            value = value * value;
        }
        out_color = vec4(vec3(value), 1);
    } else if (DEBUG_DISPLAY == DEBUG_DISPLAY_NORMAL) {
        out_color = vec4(-0.5 * in_normal + vec3(0.5), 1);
    } else {
        // An obnoxious result for an invalid value
        out_color = vec4(fract(gl_FragCoord.xyz / 16), 1);
    }
}
