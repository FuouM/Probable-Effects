// Print pass from probable-video-editor projects/breaked_neuro/video/scenes/_kit.ts (PRINT),
// without the plate overlays. Values the original reads from 8-bit canvas layers are read here
// from the display-encoded (sRGB) image.

struct U {
    ink: vec3<f32>,     // linear
    cell: f32,          // halftone cell in full-res px
    paper: vec3<f32>,   // linear
    soft: f32,
    ground: vec3<f32>,  // linear
    bleed: f32,
    scale: vec2<f32>,   // host downsample factor
    size: vec2<f32>,    // canvas px
    seed: f32,
    texture: f32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var src: texture_2d<f32>;

fn screen_at(p: vec2<f32>, cell: f32, ang: f32) -> f32 {
    let q = rot2(ang) * p / cell;
    return 0.5 + 0.25 * (cos(TAU * q.x) + cos(TAU * q.y));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Full-res px, y up like gl_FragCoord in pve, so the screens keep their orientation.
    let px = vec2(in.pos.x, u.size.y - in.pos.y) / u.scale;
    let s = u.seed;

    // Ink-edge wander: slow fbm warp of `bleed` px plus a fine 0.7 px shiver.
    let w = vec2(fbm2(px * 0.028 + vec2(s * 1.7, 3.1), 3), fbm2(px * 0.028 + vec2(31.0, s * 1.3), 3));
    let w2 = vec2(snoise2(px * 0.21 + s * 3.0), snoise2(px * 0.21 + 9.0 - s * 3.0));
    let off = w * u.bleed + w2 * 0.7;
    let uv = (in.pos.xy + vec2(off.x, -off.y) * u.scale) / u.size;
    let c = textureSampleLevel(src, samp, uv, 0.0);

    let cell = u.cell;
    let soft = max(u.soft, 1e-4);
    // Coverage: halftone screen on partial alpha, solid at full alpha.
    let a = remap(c.a, 0.06, 0.94, -0.08, 1.08);
    let cov = smoothstep(-soft, soft, a - screen_at(px, cell, 0.785));

    // Tone: perceptual luma through a second screen. Only true black is clean ink; dark greys
    // screen as paper dots again so they do not vanish on black.
    let rgb = to_srgb(unpremul(c));
    let lum = sat(luma(rgb));
    let is_dark = step(lum, 0.005);
    let tone = remap(pow(lum, 1.0 / 2.2), 0.12, 0.9, -0.08, 1.08);
    let paper_cov = (1.0 - is_dark) * smoothstep(-soft, soft, tone - screen_at(px, cell, 0.26));

    let paper_col = u.paper * (0.93 + 0.07 * u.texture * fbm2(px * 0.013 + 4.0, 2));
    let ground = u.ground * (0.88 + 0.12 * u.texture * fbm2(px * 0.006 + 2.0, 3));
    let is_ground = step(0.35, rgb.b - max(rgb.r, rgb.g));
    let fg = mix(mix(u.ink, paper_col, paper_cov), ground, is_ground);
    return vec4(mix(ground, fg, cov), 1.0);
}
