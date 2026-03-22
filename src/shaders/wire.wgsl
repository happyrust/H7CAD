// Wire shader — renders 1-D CAD entities as screen-aligned quads.
// Topology: TriangleList (6 vertices per segment).
//
// Each vertex carries both segment endpoints (pos_a, pos_b) plus which_end
// (0=A, 1=B) and side (±1).  The vertex shader expands the quad by half_width
// pixels perpendicular to the segment direction in screen space.
//
// Linetype is applied entirely on the GPU:
//   • distance = cumulative arc-length from wire start (interpolated by GPU).
//   • pattern_length > 0 enables the dash test; 0 = solid (no discard).
//   • pat0/pat1 encode up to 8 elements: positive=dash, negative=gap.

struct Uniforms {
    view_proj:     mat4x4<f32>,
    camera_pos:    vec4<f32>,
    viewport_size: vec2<f32>,
    _pad:          vec2<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexIn {
    @location(0) pos_a:          vec3<f32>,
    @location(1) pos_b:          vec3<f32>,
    @location(2) which_end:      f32,
    @location(3) side:           f32,
    @location(4) color:          vec4<f32>,
    @location(5) distance:       f32,
    @location(6) half_width:     f32,
    @location(7) pattern_length: f32,
    // location 8 = _pad (not needed in shader)
    @location(8) pat0:           vec4<f32>,
    @location(9) pat1:           vec4<f32>,
}

struct VertexOut {
    @builtin(position) clip_pos:       vec4<f32>,
    @location(0)       color:          vec4<f32>,
    @location(1)       distance:       f32,
    @location(2)       pattern_length: f32,
    @location(3)       pat0:           vec4<f32>,
    @location(4)       pat1:           vec4<f32>,
}

@vertex fn vs_main(in: VertexIn) -> VertexOut {
    let clip_a = u.view_proj * vec4<f32>(in.pos_a, 1.0);
    let clip_b = u.view_proj * vec4<f32>(in.pos_b, 1.0);

    // NDC of both endpoints.
    let ndc_a = clip_a.xy / clip_a.w;
    let ndc_b = clip_b.xy / clip_b.w;

    // Screen-space pixel positions.
    let screen_a = ndc_a * u.viewport_size * 0.5;
    let screen_b = ndc_b * u.viewport_size * 0.5;

    // Screen-space perpendicular to segment direction.
    let seg = screen_b - screen_a;
    let seg_len = length(seg);
    var perp: vec2<f32>;
    if seg_len > 1e-4 {
        let dir = seg / seg_len;
        perp = vec2<f32>(-dir.y, dir.x);
    } else {
        perp = vec2<f32>(0.0, 1.0);
    }

    // Convert perpendicular from screen pixels to NDC offset.
    let perp_ndc = perp / (u.viewport_size * 0.5);

    // Select the clip-space position for this vertex's endpoint.
    let clip_pos = mix(clip_a, clip_b, in.which_end);

    // Offset in clip space (multiply by w to un-apply perspective division).
    let ndc_offset = perp_ndc * in.half_width * in.side;
    let final_clip = clip_pos + vec4<f32>(ndc_offset * clip_pos.w, 0.0, 0.0);

    var out: VertexOut;
    out.clip_pos       = final_clip;
    out.color          = in.color;
    out.distance       = in.distance;
    out.pattern_length = in.pattern_length;
    out.pat0           = in.pat0;
    out.pat1           = in.pat1;
    return out;
}

// Returns true if arc-length `dist` falls inside a dash segment.
fn in_dash(dist: f32, pat_len: f32, p0: vec4<f32>, p1: vec4<f32>) -> bool {
    let d   = ((dist % pat_len) + pat_len) % pat_len;
    var pos = 0.0f;
    let elems = array<f32, 8>(p0.x, p0.y, p0.z, p0.w, p1.x, p1.y, p1.z, p1.w);
    for (var i = 0u; i < 8u; i++) {
        let elem = elems[i];
        if elem == 0.0 { break; }
        let len = abs(elem);
        if d < pos + len { return elem > 0.0; }
        pos += len;
    }
    return false;
}

@fragment fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    if in.pattern_length > 0.0 {
        if !in_dash(in.distance, in.pattern_length, in.pat0, in.pat1) {
            discard;
        }
    }
    return in.color;
}
