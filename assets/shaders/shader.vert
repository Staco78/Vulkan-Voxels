#version 450

layout(binding = 0) uniform UniformBufferObject {
    mat4 view;
    mat4 proj;
} ubo;

layout(location = 0) in ivec3 inPosition;
layout(location = 1) in lowp uvec3 inColor;
layout(location = 2) in lowp uint lightModifier;

layout(location = 0) out vec3 fragColor;

void main() {
    gl_Position = ubo.proj * ubo.view * vec4(inPosition, 1.0);
    fragColor = (vec3(inColor) / 256.0) * (lightModifier / 10.0);
}
