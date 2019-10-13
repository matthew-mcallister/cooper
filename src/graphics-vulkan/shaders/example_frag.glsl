#version 450
#pragma shader_stage(fragment)

#include "example_inc.glsl"

layout(location = 0) in vec3 in_world_pos;
layout(location = 1) in vec3 in_world_normal;

layout(location = 0) out vec4 out_color;

void main() {
    vec4 pos = vec4(in_world_pos, 1);
    vec4 normal = vec4(normalize(in_world_normal), 0);

    vec4 view_dir = normalize(globals.camera.pos - pos);
    vec4 half_dir = normalize(-globals.light_dir + view_dir);
    float spec_angle = max(dot(half_dir, normal), 0.0);
    float specular_power = pow(spec_angle, PHONG_SHININESS);
    vec3 specular = vec3(specular_power);

    vec3 cool_power = globals.cool_power.xyz;
    vec3 warm_power = globals.warm_power.xyz;
    float diff_angle = max(dot(globals.light_dir, normal), 0.0);
    float weight = 0.5 - 0.5 * dot(normal, globals.light_dir);
    vec3 diffuse = mix(cool_power, warm_power, weight);

    vec3 ambient = globals.ambient_power.xyz;
    
    out_color = vec4(ambient + diffuse + specular, 1);
}
