// Chromatic aberration from probable-video-editor post.ts:
//   off = dc * r2 * ca / width * 4;  R at uv + off, G at uv, B at uv - off

struct U {
    offset: f32,
    aspect: f32,
    _pad: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var src: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dc = in.uv - 0.5;
    let a = dc * vec2(u.aspect, 1.0);
    let off = dc * dot(a, a) * u.offset;
    let r = textureSampleLevel(src, samp, in.uv + off, 0.0);
    let g = textureSampleLevel(src, samp, in.uv, 0.0);
    let b = textureSampleLevel(src, samp, in.uv - off, 0.0);
    // Premultiplied: each channel keeps its own coverage; the fringe is as opaque as its strongest channel.
    return vec4(r.r, g.g, b.b, max(r.a, max(g.a, b.a)));
}
