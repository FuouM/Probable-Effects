// Halation composite: the tinted luma of the coarse pyramid level added over the input
// (pve: col += vec3(1.0, 0.18, 0.04) * luma(ha) * halation).

struct U {
    tint: vec3<f32>,    // linear
    intensity: f32,
    mode: u32,          // 0 composite over input, 1 halation only
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var halo_tex: texture_2d<f32>;
@group(0) @binding(3) var base_tex: texture_2d<f32>;

@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {
    let halo = luma(textureSampleLevel(halo_tex, samp, in.uv, 0.0).rgb);
    let glow = u.tint * (halo * u.intensity);
    let cover = clamp(max(glow.r, max(glow.g, glow.b)), 0.0, 1.0);
    if u.mode == 1u {
        return vec4(glow, cover);
    }
    let base = textureLoad(base_tex, vec2<i32>(in.pos.xy), 0);
    return vec4(base.rgb + glow, base.a + (1.0 - base.a) * cover);
}
