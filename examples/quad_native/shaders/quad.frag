#version 460
layout(location = 0) out vec4 fragColor;

layout(binding = 0) uniform MaterialUniform { vec3 tint; }
material;

void main() { fragColor = vec4(material.tint, 1.0f); }
