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
    @location(3) border_radius: vec4<f32>,
    @location(4) glow_radius: f32,
    @location(5) gradient_color: vec4<f32>,
    @location(6) glow_color: vec4<f32>,
    @location(7) gradient_mode: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) size: vec2<f32>,
    @location(3) border_radius: vec4<f32>,
    @location(4) glow_radius: f32,
    @location(5) gradient_color: vec4<f32>,
    @location(6) glow_color: vec4<f32>,
    @location(7) gradient_mode: f32,
}

// sRGB <-> linear conversion for correct gradient interpolation.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        return c / 12.92;
    }
    return pow((c + 0.055) / 1.055, 2.4);
}

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        return c * 12.92;
    }
    return 1.055 * pow(c, 1.0 / 2.4) - 0.055;
}

fn srgb_to_linear3(c: vec3<f32>) -> vec3<f32> {
    return vec3(srgb_to_linear(c.r), srgb_to_linear(c.g), srgb_to_linear(c.b));
}

fn linear_to_srgb3(c: vec3<f32>) -> vec3<f32> {
    return vec3(linear_to_srgb(c.r), linear_to_srgb(c.g), linear_to_srgb(c.b));
}

// Per-corner signed distance for a rounded box (Inigo Quilez).
// p: point relative to center of box (NOT abs)
// b: half-size of box
// r: corner radii (TL, TR, BR, BL)
fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: vec4<f32>) -> f32 {
    // Select radius based on quadrant:
    // p.x > 0 → right (TR,BR), else left (TL,BL)
    // p.y > 0 → bottom, else top
    var rs = select(r.xw, r.yz, p.x > 0.0);
    let radius = select(rs.x, rs.y, p.y > 0.0);
    let q = abs(p) - b + vec2(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2(0.0))) - radius;
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
    out.gradient_mode = instance.gradient_mode;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Output premultiplied alpha for correct compositing with PreMultiplied mode.
    let half = in.size * 0.5;

    // Base color: apply gradient with linear-space interpolation.
    // gradient_mode: 0=none, 1=vertical, 2=horizontal, 3=radial.
    var base_color = in.color;
    let gm = u32(in.gradient_mode);
    if gm > 0u && in.gradient_color.a > 0.001 {
        var t: f32;
        if gm == 2u {
            // Horizontal: left → right
            t = in.local_pos.x / max(in.size.x, 0.001);
        } else if gm == 3u {
            // Radial: center → edges
            let center = in.size * 0.5;
            let d = length((in.local_pos - center) / max(center, vec2(0.001)));
            t = clamp(d, 0.0, 1.0);
        } else {
            // Vertical (default): top → bottom
            t = in.local_pos.y / max(in.size.y, 0.001);
        }
        let from_linear = srgb_to_linear3(in.color.rgb);
        let to_linear = srgb_to_linear3(in.gradient_color.rgb);
        let mixed = mix(from_linear, to_linear, t);
        base_color = vec4(linear_to_srgb3(mixed), mix(in.color.a, in.gradient_color.a, t));
    }

    // Glow mode: quad is expanded by glow_radius, inner rect is the logical quad
    if in.glow_radius > 0.5 {
        let gr = in.glow_radius;
        let inner_size = in.size - vec2(2.0 * gr);
        let inner_half = max(inner_size * 0.5, vec2(0.0));
        let center = half;
        let p = in.local_pos - center;
        let inner_max_r = min(inner_half.x, inner_half.y);
        let inner_r = min(in.border_radius, vec4(inner_max_r));
        let d = sd_rounded_box(p, inner_half, inner_r);

        if d <= 0.0 {
            // Inside the inner rect — render base color with adaptive SDF anti-aliasing
            let fw = max(fwidth(d), 0.001);
            let edge_alpha = 1.0 - smoothstep(-fw, fw, d);
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

    // Standard rounded rect with per-corner radii (SDF)
    let any_radius = max(max(in.border_radius.x, in.border_radius.y), max(in.border_radius.z, in.border_radius.w));
    if any_radius > 0.0 {
        let max_r = min(half.x, half.y);
        let r = min(in.border_radius, vec4(max_r));
        let p = in.local_pos - half;
        let d = sd_rounded_box(p, half, r);
        let fw = max(fwidth(d), 0.001);
        let edge_alpha = 1.0 - smoothstep(-fw, fw, d);
        if edge_alpha < 0.01 {
            discard;
        }
        let a = base_color.a * edge_alpha;
        return vec4<f32>(base_color.rgb * a, a);
    }

    // Plain quad
    return vec4<f32>(base_color.rgb * base_color.a, base_color.a);
}
