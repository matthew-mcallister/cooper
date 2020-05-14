const uint CONST_ID_SIMPLE_MODE = 0;

const uint SIMPLE_MODE_CHECKER = 0;
const uint SIMPLE_MODE_DEPTH = 1;
const uint SIMPLE_MODE_NORMAL = 2;

const uint VERTEX_ATTR_POSITION = 0;
const uint VERTEX_ATTR_NORMAL = 1;

struct Perspective {
    mat4 proj;
    //mat4 proj_inv;
    float tan_fovx2;
    float tan_fovy2;
    float z_near;
    float z_far;
    float min_depth;
    float max_depth;
};

struct SceneView {
    Perspective perspective;
    mat4 view;
    mat4 view_inv;
};

struct Instance {
    vec4 xform[3];
};

// Good for debugging
mat4 perspective(
    float s_x, float s_y,
    float z_n, float z_f,
    float d_n, float d_f)
{
    float c = z_f * (d_f - d_n) / (z_f - z_n);
    return mat4(
        1.0 / s_x, 0.0,       0.0,      0.0,
        0.0,       1.0 / s_y, 0.0,      0.0,
        0.0,       0.0,       c + d_n,  1.0,
        0.0,       0.0,       -z_n * c, 0.0);
}
