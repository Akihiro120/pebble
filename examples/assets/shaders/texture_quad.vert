#version 460
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec2 aTexCoords;

layout(location = 0) out vec3 vPos;
layout(location = 1) out vec2 vTexCoords;

void main() {
    vPos = aPos;
    vTexCoords = aTexCoords;

    gl_Position = vec4(aPos, 1.0f);
}
