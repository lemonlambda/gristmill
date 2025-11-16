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
    float DELTA = 3.0;
    float s = sqrt(float(sporadic.num_instances));
    float c = ceil(float(s));
    int cols = int(c);
    int full_rows = gl_InstanceIndex / cols;
    int remainder = gl_InstanceIndex % cols;

    float xdelta = DELTA * float(remainder);
    float ydelta = DELTA * float(full_rows);

    fragColor = vec3(1.0, float(1.0 + gl_InstanceIndex) / float(sporadic.num_instances), 0.0);

    vec4 vertex = vec4(inPosition.xyz + vec3(xdelta, ydelta, 0), 1.0);
    // gl_Position = ubo.proj * ubo.view * pcs.model * vertex;
    gl_Position = vec4(inPosition, 1.0);
    fragTexCoord = inTexCoord;
}

