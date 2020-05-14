#version 450
#pragma shader_stage(fragment)

#include "common_inc.glsl"

// TODO: Consider collecting all spec constants in one big header
layout(constant_id = CONST_ID_SIMPLE_MODE)
const uint SIMPLE_MODE = SIMPLE_MODE_DEPTH;

layout(set = 0, binding = 0) uniform blk_scene {
    SceneView g_scene_view;
};

layout(location = 0) in vec3 in_normal;

layout(location = 0) out vec4 out_color;

void main() {
    if (SIMPLE_MODE == SIMPLE_MODE_DEPTH) {
        // TODO: configurable saturation point for interior scenes
        float z = 1 / gl_FragCoord.w;
        float z_far = g_scene_view.perspective.z_far;
        float z_near = g_scene_view.perspective.z_near;
        // Invert to mimic the real Z buffer
        float value = 1.0 - z / (z_far - z_near);
        out_color = vec4(vec3(value), 1);
    } else if (SIMPLE_MODE == SIMPLE_MODE_NORMAL) {
        vec3 color = -0.5 * in_normal + vec3(0.5);
        out_color = vec4(color, 1);
    } else {
        // Display the checker pattern by default
        vec4 colors[2] = { vec4(1.0, 0.0, 0.0, 1.0), vec4(vec3(0.5), 1.0) };
        uvec2 coord = uvec2(gl_FragCoord.xy) / 8;
        uint t = (coord.x + coord.y) % 2;
        vec4 color = mix(colors[0], colors[1], float(t));
        out_color = vec4(color.xyz, 1);
    }
}
