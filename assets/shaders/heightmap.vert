#version 450

layout(location = 0) out float v_height;
layout(location = 1) out vec2 v_Uv;
layout(location = 2) out vec3 v_Pos;

layout(location = 0) in vec3 Vertex_Position;
layout(location = 1) in vec2 Vertex_Uv;

layout(set = 0, binding = 0) uniform CameraViewProj {
  mat4 ViewProj;
};

layout(set = 1, binding = 0) uniform Transform {
  mat4 Model;
};

layout(set = 2, binding = 0) uniform texture2D heightmap;
layout(set = 2, binding = 1) uniform sampler heightmap_sampler;

float terrain(vec2 pos) {
  vec4 h = texture(sampler2D(heightmap, heightmap_sampler), pos.yx);
	return h.r * 20;
}

void main() {
	float height = terrain(Vertex_Uv);
  vec3 pos = Vertex_Position;
	pos.y = height;

  v_height = height;
  v_Uv = Vertex_Uv;
	v_Pos = Vertex_Position;
  gl_Position = ViewProj * Model * vec4(pos, 1.0);
}
