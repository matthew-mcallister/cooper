#version 450
#pragma shader_stage(fragment)

#include "common_inc.glsl"

// TODO: Consider collecting all spec constants in one big header
layout(constant_id = CONST_ID_DEBUG_DISPLAY)
const uint DEBUG_DISPLAY = DEBUG_DISPLAY_DEPTH;

layout(set = 0, binding = 0) uniform blk_scene {
    SceneView g_scene_view;
};
layout(set = 0, binding = 1) readonly buffer blk_inst {
    DebugInstance g_instances[];
};

layout(location = 0) flat in uint in_instance_index;
layout(location = 1) in vec3 in_normal;

layout(location = 0) out vec4 out_color;

void main() {
    if (DEBUG_DISPLAY == DEBUG_DISPLAY_DEPTH) {
        // TODO: configurable saturation point for interior scenes
        float z = 1 / gl_FragCoord.w;
        float z_far = g_scene_view.perspective.z_far;
        float z_near = g_scene_view.perspective.z_near;
        // Invert to mimic the real Z buffer
        float value = 1.0 - z / (z_far - z_near);
        out_color = vec4(vec3(value), 1);
    } else if (DEBUG_DISPLAY == DEBUG_DISPLAY_NORMAL) {
        vec3 color = -0.5 * in_normal + vec3(0.5);
        out_color = vec4(color, 1);
    } else {
        // Display the checker pattern by default
        DebugInstance inst = g_instances[in_instance_index];
        vec4 colors[2] = inst.colors;
        uvec2 coord = uvec2(gl_FragCoord.xy) / 8;
        uint t = (coord.x + coord.y) % 2;
        vec4 color = mix(colors[0], colors[1], float(t));
        out_color = vec4(color.xyz, 1);
    }
}
