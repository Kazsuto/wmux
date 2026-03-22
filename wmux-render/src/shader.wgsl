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
    @location(3) border_radius: f32,
    @location(4) glow_radius: f32,
    @location(5) gradient_color: vec4<f32>,
    @location(6) glow_color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) border_radius: f32,
    @location(4) glow_radius: f32,
    @location(5) gradient_color: vec4<f32>,
    @location(6) glow_color: vec4<f32>,
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
    out.local_pos = offset * instance.size;
    out.size = instance.size;
    out.border_radius = instance.border_radius;
    out.glow_radius = instance.glow_radius;
    out.gradient_color = instance.gradient_color;
    out.glow_color = instance.glow_color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Output premultiplied alpha for correct compositing with PreMultiplied mode.
    let half = in.size * 0.5;

    // Base color: apply vertical gradient when gradient_color.a > 0
    var base_color = in.color;
    if in.gradient_color.a > 0.001 {
        let t = in.local_pos.y / max(in.size.y, 0.001);
        base_color = mix(in.color, in.gradient_color, t);
    }

    // Glow mode: quad is expanded by glow_radius, inner rect is the logical quad
    if in.glow_radius > 0.5 {
        let gr = in.glow_radius;
        let inner_size = in.size - vec2(2.0 * gr);
        let inner_half = max(inner_size * 0.5, vec2(0.0));
        let center = half;
        let p = abs(in.local_pos - center) - inner_half;
        let radius = min(in.border_radius, min(inner_half.x, inner_half.y));
        let p_rounded = p + vec2(radius);
        let d = length(max(p_rounded, vec2(0.0))) - radius;

        if d <= 0.0 {
            // Inside the inner rect — render base color with SDF anti-aliasing
            let edge_alpha = 1.0 - smoothstep(-0.5, 0.5, d);
            let a = base_color.a * edge_alpha;
            return vec4<f32>(base_color.rgb * a, a);
        } else {
            // Outside inner rect — glow falloff
            let glow_t = 1.0 - smoothstep(0.0, gr, d);
            let ga = in.glow_color.a * glow_t * glow_t; // quadratic falloff for softer glow
            if ga < 0.003 {
                discard;
            }
            return vec4<f32>(in.glow_color.rgb * ga, ga);
        }
    }

    // Standard rounded rect (SDF)
    if in.border_radius > 0.0 {
        let p = abs(in.local_pos - half) - half + vec2(in.border_radius);
        let d = length(max(p, vec2(0.0))) - in.border_radius;
        let edge_alpha = 1.0 - smoothstep(-0.5, 0.5, d);
        if edge_alpha < 0.01 {
            discard;
        }
        let a = base_color.a * edge_alpha;
        return vec4<f32>(base_color.rgb * a, a);
    }

    // Plain quad
    return vec4<f32>(base_color.rgb * base_color.a, base_color.a);
}
