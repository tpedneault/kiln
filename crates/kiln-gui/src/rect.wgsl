// Rectangles: bars, bands, the cursor's row, the popup, and status pills. Colours are written as they
// are, onto a non-sRGB surface, the same as Fenix's `rect.wgsl` for a plain rect. A rect with a radius
// has rounded corners, cut with a rounded-box distance in pixels and smoothed over one pixel.

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    // x, y, width, height in physical pixels, for the corners.
    @location(2) rect: vec4<f32>,
    @location(3) radius: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) rect: vec4<f32>,
    @location(2) radius: f32,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(in.position, 0.0, 1.0);
    out.color = in.color;
    out.rect = in.rect;
    out.radius = in.radius;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    if (in.radius <= 0.0) {
        return in.color;
    }
    let half = in.rect.zw * 0.5;
    let center = in.rect.xy + half;
    let q = abs(in.clip_position.xy - center) - (half - vec2<f32>(in.radius));
    let d = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - in.radius;
    let coverage = clamp(0.5 - d, 0.0, 1.0);
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
