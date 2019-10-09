#version 450
#pragma shader_stage(fragment)

#include "example_inc.glsl"

layout(location = 0) in vec4 in_world_pos;
layout(location = 1) in vec4 in_world_normal;

layout(location = 0) out vec4 out_color;

void main() {
    vec4 normal = normalize(in_world_normal);
    
    vec4 view_dir = normalize(globals.camera.pos - in_world_pos);
    vec4 half_dir = normalize(globals.light_dir + view_dir);
    float spec_angle = max(dot(half_dir, normal), 0.0);
    float specular = pow(spec_angle, PHONG_SHININESS);

    float diff_angle = max(dot(globals.light_dir, normal), 0.0);
    float weight = 0.5 - 0.5 * dot(normal, globals.light_dir);
    vec4 diffuse = mix(globals.cool_power, globals.warm_power, weight);
    
    vec4 ambient = globals.ambient_power;
    
    out_color = ambient + diffuse + specular;
}
