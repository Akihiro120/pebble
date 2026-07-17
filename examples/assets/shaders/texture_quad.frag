#version 460
layout(location = 0) out vec4 fragColor;

layout(location = 0) in vec3 vPos;
layout(location = 1) in vec2 vTexCoords;

layout(set = 0, binding = 0) uniform texture2D tAlbedo;
layout(set = 0, binding = 1) uniform sampler sAlbedo;

void main() {
    vec3 color = texture(sampler2D(tAlbedo, sAlbedo), vTexCoords).rgb;
    fragColor = vec4(color, 1.0f);
}
