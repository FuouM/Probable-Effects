// Corner crop marks (probable-video-editor hud.ts, Hud.cropMarks). Each corner is an L:
//   moveTo(x + sx*l, y + 0.5*sy) -> lineTo(x, y + 0.5*sy) -> lineTo(x, y + sy*l)
// with (x, y) the inset corner and (sx, sy) pointing into the frame.

struct U {
    colour: vec3<f32>,  // linear
    alpha: f32,
    size: vec2<f32>,    // canvas px
    inset: f32,
    length: f32,
    half_width: f32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;

fn dist_segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let pa = p - a;
    let ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}

fn corner(p: vec2<f32>, at: vec2<f32>, dir: vec2<f32>) -> f32 {
    let knee = at + vec2(0.0, 0.5 * dir.y);
    let d = min(
        dist_segment(p, vec2(at.x + dir.x * u.length, knee.y), knee),
        dist_segment(p, knee, vec2(at.x, at.y + dir.y * u.length)),
    );
    // One pixel of antialiasing around the half-width.
    return sat(u.half_width - d + 0.5);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let base = textureLoad(src, vec2<i32>(in.pos.xy), 0);
    let p = in.pos.xy;
    let m = u.inset;
    let far = u.size - vec2(m);
    var cover = corner(p, vec2(m, m), vec2(1.0, 1.0));
    cover = max(cover, corner(p, vec2(far.x, m), vec2(-1.0, 1.0)));
    cover = max(cover, corner(p, vec2(m, far.y), vec2(1.0, -1.0)));
    cover = max(cover, corner(p, far, vec2(-1.0, -1.0)));
    let a = cover * u.alpha;
    return vec4(base.rgb * (1.0 - a) + u.colour * a, base.a + (1.0 - base.a) * a);
}
