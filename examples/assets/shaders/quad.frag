#version 460
layout(location = 0) out vec4 fragColor;

layout(location = 0) in vec3 vPos;

void main() {
    fragColor = vec4(vPos, 1.0f);
}
