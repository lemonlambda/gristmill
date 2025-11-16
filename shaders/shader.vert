#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
} ubo;

layout(binding = 1) uniform SporadicBufferObject {
    int num_instances;
} sporadic;

layout(push_constant) uniform PushConstants {
    mat4 model;
} pcs;

layout(location = 0) in vec3 inPosition;
layout(location = 1) in vec3 inColor;
layout(location = 2) in vec2 inTexCoord;

layout(location = 0) out vec3 fragColor;
layout(location = 1) out vec2 fragTexCoord;

void main() {

    vec4 vertex = vec4(inPosition.xyz + vec3(gl_InstanceIndex, 0.0, 0.0), 1.0);
    gl_Position = ubo.proj * ubo.view * pcs.model * vertex;
    fragColor = inColor;
    fragTexCoord = inTexCoord;
}

