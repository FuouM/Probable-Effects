// Vignette from probable-video-editor post.ts:
//   v = smoothstep(0.95, 0.25, length((uv - 0.5) * vec2(1, 0.8))); col *= mix(1, v, vignette)
// (WGSL leaves smoothstep with edge0 >= edge1 undefined, so it is written as 1 - smoothstep.)

struct U {
    amount: f32,
    inner: f32,
    outer: f32,
    squeeze: f32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = textureLoad(src, vec2<i32>(in.pos.xy), 0);
    let d = length((in.uv - 0.5) * vec2(1.0, u.squeeze));
    let v = 1.0 - smoothstep(min(u.inner, u.outer), max(u.inner, u.outer + 1e-4), d);
    return vec4(t.rgb * mix(1.0, v, u.amount), t.a);
}
