struct TrivialInstance {
    vec4 radius;
    vec4 offs;
    vec4 rot_cols[3];
    vec4 colors[8];
};

layout(set = 0, binding = 0) uniform blk_scene_globals {
    SceneGlobals g_globals;
};

layout(set = 1, binding = 0) readonly buffer blk_instances {
    TrivialInstance g_instances[];
};

TrivialInstance get_inst() {
    return g_instances[gl_InstanceIndex];
}
