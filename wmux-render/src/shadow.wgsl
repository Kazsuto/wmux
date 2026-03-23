// Analytical drop shadows using the Evan Wallace (Figma) erf() approximation.
// Each shadow is a single expanded quad; the fragment shader computes the
// Gaussian-convolved box shadow analytically — no blur passes needed.

struct Viewport {
    width: f32,
    height: f32,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> viewport: Viewport;

struct ShadowInput {
    @location(0) pos: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) caster_pos: vec2<f32>,
    @location(4) caster_size: vec2<f32>,
    @location(5) sigma: f32,
    @location(6) border_radius: f32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) caster_pos: vec2<f32>,
    @location(3) caster_size: vec2<f32>,
    @location(4) sigma: f32,
    @location(5) border_radius: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32, instance: ShadowInput) -> VertexOutput {
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

    let ndc = vec2<f32>(
        pixel.x / viewport.width * 2.0 - 1.0,
        1.0 - pixel.y / viewport.height * 2.0,
    );

    var out: VertexOutput;
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = instance.color;
    out.local_pos = offset * instance.size;
    out.caster_pos = instance.caster_pos;
    out.caster_size = instance.caster_size;
    out.sigma = instance.sigma;
    out.border_radius = instance.border_radius;
    return out;
}

// erf() approximation — 4-term polynomial (max error ~0.05%).
// Standard in GPUI (Zed) and Figma's renderer.
fn erf_approx(x: f32) -> f32 {
    let s = sign(x);
    let a = abs(x);
    let t = 1.0 + (0.278393 + (0.230389 + 0.078108 * (a * a)) * a) * a;
    let r = t * t;
    return s - s / (r * r);
}

// 1D Gaussian integral over [lower, upper] evaluated at point.
fn box_shadow_1d(lower: f32, upper: f32, point: f32, inv_sigma: f32) -> f32 {
    let a = 0.5 + 0.5 * erf_approx((point - lower) * inv_sigma);
    let b = 0.5 + 0.5 * erf_approx((upper - point) * inv_sigma);
    return a + b - 1.0;
}

// 2D box shadow via separable 1D integrals.
fn box_shadow(lower: vec2<f32>, upper: vec2<f32>, point: vec2<f32>, sigma: f32) -> f32 {
    let inv_sigma = 0.7071067811865476 / max(sigma, 0.001); // sqrt(0.5) / sigma
    return box_shadow_1d(lower.x, upper.x, point.x, inv_sigma)
         * box_shadow_1d(lower.y, upper.y, point.y, inv_sigma);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let lower = in.caster_pos;
    let upper = in.caster_pos + in.caster_size;

    let intensity = box_shadow(lower, upper, in.local_pos, in.sigma);

    if intensity < 0.003 {
        discard;
    }

    let a = in.color.a * intensity;
    return vec4<f32>(in.color.rgb * a, a);
}
