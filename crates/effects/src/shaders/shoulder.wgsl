// Tone shoulder from probable-video-editor post.ts (SHOULDER_GLSL), applied to straight colour.

struct U {
    exposure: f32,
    start: f32,     // k: identity below, shoulder above
    white: f32,     // how far values above 2..12 roll to white
    _pad: f32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;

fn shoulder(x: vec3<f32>) -> vec3<f32> {
    let k = u.start;
    let curve = vec3(k) + (1.0 - k) * (1.0 - exp(-(x - vec3(k)) / (1.0 - k)));
    let y = select(x, curve, x >= vec3(k));
    let over = max(max(x.r, x.g), x.b);
    return mix(y, vec3(1.0), smoothstep(2.0, 12.0, over) * u.white);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let t = textureLoad(src, vec2<i32>(in.pos.xy), 0);
    return vec4(shoulder(unpremul(t) * u.exposure) * t.a, t.a);
}
