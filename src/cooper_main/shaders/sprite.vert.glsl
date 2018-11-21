#version 450

#pragma shader_stage(vertex)

layout(constant_id = 0) const float INV_ASPECT_RATIO = 9.0f / 16.0f;

// One of the corners of the unit square
layout(location = 0) in vec2 in_vert_pos;

// Top-left corner position
layout(location = 10) in vec2 in_inst_pos;
// Height in normalized screen units
layout(location = 11) in float in_inst_height;

layout(location = 0) out vec2 out_tex_coord;

void main() {
    vec2 scale = in_inst_height * vec2(INV_ASPECT_RATIO, -1.0f);
    vec2 pos = in_inst_pos + scale * in_inst_pos;

    gl_Position = vec4(pos, 0.0f, 1.0f);
    out_tex_coord = in_vert_pos;
}
