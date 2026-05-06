// Face3D shader — flat-shaded triangle fill for DXF 3DFACE entities.
//
// Vertex layout (28 bytes):
//   position  [f32; 3]   offset  0   12 B
//   color     [f32; 4]   offset 12   16 B

struct Uniforms {
    view_proj: mat4x4<f32>,
    viewport:  vec2<f32>,
    _pad:      vec2<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

struct VertexIn {
    @location(0) position: vec3<f32>,
    @location(1) color:    vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       color:    vec4<f32>,
};

@vertex
fn vs_main(v: VertexIn) -> VertexOut {
    var out: VertexOut;
    out.clip_pos = u.view_proj * vec4<f32>(v.position, 1.0);
    out.color    = v.color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return in.color;
}
