// Film grain from probable-video-editor post.ts: two hash scales, amplitude peaking in the
// mid-tones, added in sRGB so `amount` means the same as in pve.

struct U {
    amount: f32,
    seed_fine: f32,
    seed_coarse: f32,
    _pad: f32,
    scale: vec2<f32>,
    _pad2: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = textureLoad(src, vec2<i32>(in.pos.xy), 0);
    // Full-resolution pixel coordinates: a half-res preview shows the same grain field.
    let px = in.pos.xy / u.scale;
    let g1 = hash12(px + u.seed_fine) - 0.5;
    let g2 = hash12(floor(px / 2.0) + u.seed_coarse) - 0.5;

    var s = to_srgb(unpremul(t));
    let lm = luma(sat3(s));
    let amt = u.amount * (0.55 + 1.2 * lm * (1.0 - lm));
    s += (g1 * 0.6 + g2 * 0.4) * amt;
    return vec4(to_linear(max(s, vec3(0.0))) * t.a, t.a);
}
