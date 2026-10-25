// Bloom, ported from probable-video-editor packages/engine/src/post.ts: soft-threshold prefilter,
// 13-tap downsample pyramid (Jimenez 2014), tent upsample, additive composite.

struct P {
    texel: vec2<f32>,   // 1 / size of the texture read through `src`
    threshold: f32,
    knee: f32,
    radius: f32,        // upsample tent spread, in source texels
    intensity: f32,
    mode: u32,          // 0 composite over input, 1 bloom only
    _pad: u32,
};

@group(0) @binding(0) var<uniform> u: P;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var src: texture_2d<f32>;
@group(0) @binding(3) var prev: texture_2d<f32>;

fn tap(uv: vec2<f32>, o: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(src, samp, uv + u.texel * o, 0.0).rgb;
}

@fragment
fn fs_prefilter(in: VsOut) -> @location(0) vec4<f32> {
    // 4 bilinear taps = 4x4 source box, then a soft knee on the brightest channel.
    var c = (tap(in.uv, vec2(-1.0, -1.0)) + tap(in.uv, vec2(1.0, -1.0))
           + tap(in.uv, vec2(-1.0, 1.0)) + tap(in.uv, vec2(1.0, 1.0))) * 0.25;
    c = min(c, vec3(40.0));
    let l = max(c.r, max(c.g, c.b));
    var rq = clamp(l - u.threshold + u.knee, 0.0, 2.0 * u.knee);
    rq = rq * rq / (4.0 * u.knee + 1e-5);
    let w = max(rq, l - u.threshold) / max(l, 1e-5);
    return vec4(c * w, 1.0);
}

@fragment
fn fs_down(in: VsOut) -> @location(0) vec4<f32> {
    let a = tap(in.uv, vec2(-2.0, -2.0));
    let b = tap(in.uv, vec2(0.0, -2.0));
    let c = tap(in.uv, vec2(2.0, -2.0));
    let d = tap(in.uv, vec2(-1.0, -1.0));
    let e = tap(in.uv, vec2(1.0, -1.0));
    let f = tap(in.uv, vec2(-2.0, 0.0));
    let g = tap(in.uv, vec2(0.0, 0.0));
    let h = tap(in.uv, vec2(2.0, 0.0));
    let i = tap(in.uv, vec2(-1.0, 1.0));
    let j = tap(in.uv, vec2(1.0, 1.0));
    let k = tap(in.uv, vec2(-2.0, 2.0));
    let l = tap(in.uv, vec2(0.0, 2.0));
    let m = tap(in.uv, vec2(2.0, 2.0));
    let o = (d + e + i + j) * 0.125
          + (a + b + g + f) * 0.03125 + (b + c + h + g) * 0.03125
          + (f + g + l + k) * 0.03125 + (g + h + m + l) * 0.03125;
    return vec4(o, 1.0);
}

@fragment
fn fs_up(in: VsOut) -> @location(0) vec4<f32> {
    // 9-tap tent of the smaller level (`src`), added to this level (`prev`).
    let r = u.radius;
    let s = tap(in.uv, vec2(-r, -r)) + 2.0 * tap(in.uv, vec2(0.0, -r)) + tap(in.uv, vec2(r, -r))
          + 2.0 * tap(in.uv, vec2(-r, 0.0)) + 4.0 * tap(in.uv, vec2(0.0, 0.0)) + 2.0 * tap(in.uv, vec2(r, 0.0))
          + tap(in.uv, vec2(-r, r)) + 2.0 * tap(in.uv, vec2(0.0, r)) + tap(in.uv, vec2(r, r));
    return vec4(textureSampleLevel(prev, samp, in.uv, 0.0).rgb + s / 16.0, 1.0);
}

@fragment
fn fs_composite(in: VsOut) -> @location(0) vec4<f32> {
    // `src` = bloom (half res, bilinear), `prev` = the input canvas (same size as the target).
    let bloom = textureSampleLevel(src, samp, in.uv, 0.0).rgb * u.intensity;
    let cover = clamp(max(bloom.r, max(bloom.g, bloom.b)), 0.0, 1.0);
    if u.mode == 1u {
        return vec4(bloom, cover);
    }
    let base = textureLoad(prev, vec2<i32>(in.pos.xy), 0);
    // Premultiplied add; light spilling onto transparent pixels makes them as opaque as it is bright.
    return vec4(base.rgb + bloom, base.a + (1.0 - base.a) * cover);
}
