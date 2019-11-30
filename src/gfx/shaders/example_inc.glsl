layout(constant_id = 1) const float PHONG_SHININESS = 16.0;

struct Instance {
    vec4 pos;
    vec4 fwd;
    vec4 rgt;
    vec4 abv;
    vec4 scale;
};

struct Camera {
    mat4 perspective;
    mat4 view;
    // Columns of view^{-1}
    vec4 pos;
    vec4 fwd;
    vec4 dwn;
    vec4 rgt;
};

struct SceneGlobals {
    vec4 light_dir;
    vec4 warm_power;
    vec4 cool_power;
    vec4 ambient_power;
    Camera camera;
};

layout(set = 0, binding = 0) readonly buffer blk_scene_globals {
    SceneGlobals globals;
};
layout(set = 0, binding = 1) readonly buffer blk_instances {
    Instance g_instances[];
};
