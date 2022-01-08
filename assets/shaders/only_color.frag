#version 450

layout(location = 0) out vec4 o_Target;

layout(set = 2, binding = 0) uniform base_color {
  vec4 color;
};

void main() {
	vec4 output_color = color;
  output_color *= 0.5;
  o_Target = output_color;
}

