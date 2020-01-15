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
    Camera camera;
};
