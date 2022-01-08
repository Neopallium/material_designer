#version 450

layout(location = 0) in vec2 v_Uv;
layout(location = 0) out vec4 o_Target;

layout(set = 2, binding = 0) uniform base_color {
  vec4 color;
};
layout(set = 2, binding = 1) uniform texture2D base_texture;
layout(set = 2, binding = 2) uniform sampler base_texture_sampler;

void main() {
	vec4 output_color = color;
  output_color *= texture(sampler2D(base_texture, base_texture_sampler), v_Uv);
  o_Target = output_color;
}
