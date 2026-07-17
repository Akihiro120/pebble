#version 460
layout(location = 0) out vec4 fragColor;

layout(location = 0) in vec3 vPos;
layout(location = 1) in vec2 vTexCoords;
layout(location = 2) in vec3 vNormals;

layout(set = 1, binding = 0) uniform texture2D tAlbedo;
layout(set = 1, binding = 1) uniform sampler sAlbedo;

void main() {
    vec3 color = texture(sampler2D(tAlbedo, sAlbedo), vTexCoords).rgb;
    vec3 light_pos = vec3(1.0f, 3.0f, -2.0);
    vec3 light_color = vec3(1.0f);

    float ambient_strength = 0.2f;
    vec3 ambient = ambient_strength * light_color;

    vec3 norm = normalize(vNormals);
    vec3 light_dir = normalize(light_pos - vPos);
    float diff = max(dot(norm, light_dir), 0.0);
    vec3 diffuse = diff * light_color;

    vec3 result = (ambient + diffuse) * color;
    fragColor = vec4(result, 1.0f);
}
