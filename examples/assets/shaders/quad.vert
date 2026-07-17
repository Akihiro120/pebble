#version 460
layout(location = 0) in vec3 aPos;

layout(location = 0) out vec3 vPos;

void main() {
    vPos = aPos;
    gl_Position = vec4(aPos, 1.0f);
}
