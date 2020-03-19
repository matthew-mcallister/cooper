const uint CONST_ID_DEBUG_DISPLAY = 0;

const uint DEBUG_DISPLAY_DEPTH = 0;
const uint DEBUG_DISPLAY_NORMAL = 1;

struct Perspective {
    float tan_fovx2;
    float tan_fovy2;
    float z_near;
    float z_far;
    mat4 proj;
};

struct SceneView {
    Perspective perspective;
};
