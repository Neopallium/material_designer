#version 450

layout(location = 0) in float v_height;
layout(location = 1) in vec2 v_Uv;
layout(location = 2) in vec3 v_Pos;
layout(location = 0) out vec4 o_Target;

layout(set = 2, binding = 2) uniform texture2D details;
layout(set = 2, binding = 3) uniform sampler details_sampler;

void main() {
  vec2 uv = v_Uv;
	// fetch texture color.
  vec4 color;
	color = texture(sampler2D(details, details_sampler), uv);
  //color = vec4(v_height, 0.0, 0.0, 1.0);

	o_Target = color;
}
