// Shared helpers, prepended to every effect shader (see `shader()` in lib.rs). Ported from
// probable-video-editor packages/engine/src/glsl/common.ts; add pve functions here as effects
// need them, not in each effect's shader.

fn sat(x: f32) -> f32 { return clamp(x, 0.0, 1.0); }
fn sat3(x: vec3<f32>) -> vec3<f32> { return clamp(x, vec3(0.0), vec3(1.0)); }
fn luma(c: vec3<f32>) -> f32 { return dot(c, vec3(0.2126, 0.7152, 0.0722)); }

// Dave Hoskins' hash without sine; the same hash pve uses, so grain keeps its character.
fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    let l = max(c, vec3(0.0));
    return select(1.055 * pow(l, vec3(1.0 / 2.4)) - 0.055, 12.92 * l, l < vec3(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c < vec3(0.04045));
}

// Straight colour of a premultiplied texel; transparent texels stay black.
fn unpremul(c: vec4<f32>) -> vec3<f32> {
    return select(vec3(0.0), c.rgb / c.a, c.a > 1e-6);
}
