// Tone-driven engraving, built on pve's hatch()/engrave() (common.wgsl).

struct U {
    ink: vec3<f32>,     // linear
    freq: f32,          // line pairs per full-res px
    paper: vec3<f32>,   // linear
    angle: f32,
    scale: vec2<f32>,   // host downsample factor
    contrast: f32,
    mix_amount: f32,
    style: u32,         // 0 single hatch, 1 engrave (hairline + crosshatch)
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = textureLoad(src, vec2<i32>(in.pos.xy), 0);
    let straight = unpremul(t);

    // Perceptual tone: dark = 1. Contrast pivots around mid-grey.
    let tone = luma(sat3(to_srgb(straight)));
    let darkness = sat((1.0 - tone - 0.5) * u.contrast + 0.5);

    // Lines live on the full-res pixel grid so a half-res preview draws the same engraving.
    let px = in.pos.xy / u.scale;
    var ink_cover = 0.0;
    if u.style == 0u {
        let r = rot2(u.angle) * px * u.freq;
        ink_cover = hatch(r.y, darkness);
    } else {
        ink_cover = engrave(px, darkness, u.freq, u.angle);
    }

    let printed = mix(u.paper, u.ink, ink_cover);
    let rgb = mix(straight, printed, u.mix_amount);
    return vec4(rgb * t.a, t.a);
}
