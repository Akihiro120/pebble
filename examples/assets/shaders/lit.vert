#version 460
layout(location = 0) in vec3 aPos;
layout(location = 1) in vec2 aTexCoords;
layout(location = 2) in vec3 aNormals;

layout(location = 0) out vec3 vPos;
layout(location = 1) out vec2 vTexCoords;
layout(location = 2) out vec3 vNormals;

layout(set = 0, binding = 0) uniform Camera {
    mat4 proj;
    mat4 view;
} cam;

void main() {
    vPos = aPos;
    vTexCoords = aTexCoords;
    vNormals = aNormals;

    gl_Position = cam.proj * cam.view * vec4(aPos, 1.0f);
}
