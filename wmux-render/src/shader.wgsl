struct Viewport {
    width: f32,
    height: f32,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> viewport: Viewport;

struct QuadInput {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: QuadInput) -> VertexOutput {
    // Two triangles forming a quad: TL, TR, BL, TR, BR, BL
    var offsets = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(1.0, 0.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(0.0, 1.0),
    );

    let offset = offsets[vertex_index];
    let pixel = instance.pos + offset * instance.size;

    // Pixel coordinates to NDC: (0,0) top-left → (-1,1), (w,h) bottom-right → (1,-1)
    let ndc = vec2<f32>(
        pixel.x / viewport.width * 2.0 - 1.0,
        1.0 - pixel.y / viewport.height * 2.0,
    );

    var out: VertexOutput;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = instance.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
