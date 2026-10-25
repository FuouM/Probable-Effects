// Bloom composite. The pyramid itself is shaders/pyramid.wgsl (shared with Halation).

struct U {
    intensity: f32,
    mode: u32,          // 0 composite over input, 1 bloom only
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;   // half res, bilinear
@group(0) @binding(3) var base_tex: texture_2d<f32>;    // the input canvas, same size as the target

@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {
    let bloom = textureSampleLevel(bloom_tex, samp, in.uv, 0.0).rgb * u.intensity;
    let cover = clamp(max(bloom.r, max(bloom.g, bloom.b)), 0.0, 1.0);
    if u.mode == 1u {
        return vec4(bloom, cover);
    }
    let base = textureLoad(base_tex, vec2<i32>(in.pos.xy), 0);
    // Premultiplied add; light spilling onto transparent pixels makes them as opaque as it is bright.
    return vec4(base.rgb + bloom, base.a + (1.0 - base.a) * cover);
}
