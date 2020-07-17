const uint CONST_ID_GEOM_VIS_MODE = 0;
const uint CONST_ID_TEXTURE_VIS_SLOT = 1;

const uint VERTEX_ATTR_POSITION = 0;
const uint VERTEX_ATTR_NORMAL = 1;
const uint VERTEX_ATTR_TANGENT = 2;
const uint VERTEX_ATTR_QTANGENT = 3;
const uint VERTEX_ATTR_TEXCOORD0 = 4;
const uint VERTEX_ATTR_TEXCOORD1 = 5;
const uint VERTEX_ATTR_COLOR = 6;
const uint VERTEX_ATTR_JOINTS = 7;
const uint VERTEX_ATTR_WEIGHTS = 8;
const uint VERTEX_ATTR_VELOCITY = 9;

const uint GEOM_VIS_MODE_CHECKER = 0;
const uint GEOM_VIS_MODE_DEPTH = 1;
const uint GEOM_VIS_MODE_NORMAL = 2;

const uint IMAGE_SLOT_ALBEDO = 0;
const uint IMAGE_SLOT_NORMAL = 1;
const uint IMAGE_SLOT_METALLIC_ROUGHNESS = 2;
const uint IMAGE_SLOT_MAX = 3;

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
