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

const PI: f32 = 3.14159265359;
const TAU: f32 = 6.28318530718;

fn remap(x: f32, a: f32, b: f32, c: f32, d: f32) -> f32 { return c + (d - c) * sat((x - a) / (b - a)); }

// GLSL mat2(c, -s, s, c): `rot2(a) * p` rotates p by a, as in pve.
fn rot2(a: f32) -> mat2x2<f32> {
    let c = cos(a);
    let s = sin(a);
    return mat2x2<f32>(vec2(c, -s), vec2(s, c));
}

// ---- hashes (Dave Hoskins) ----
fn hash11(p0: f32) -> f32 {
    var p = fract(p0 * 0.1031);
    p *= p + 33.33;
    p *= p + p;
    return fract(p);
}

fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3(p.x, p.y, p.x) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

fn hash13(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

// ---- simplex noise (Ashima / Stefan Gustavson, MIT) ----
fn mod289_2(x: vec2<f32>) -> vec2<f32> { return x - floor(x * (1.0 / 289.0)) * 289.0; }
fn mod289_3(x: vec3<f32>) -> vec3<f32> { return x - floor(x * (1.0 / 289.0)) * 289.0; }
fn mod289_4(x: vec4<f32>) -> vec4<f32> { return x - floor(x * (1.0 / 289.0)) * 289.0; }
fn permute3(x: vec3<f32>) -> vec3<f32> { return mod289_3(((x * 34.0) + 10.0) * x); }
fn permute4(x: vec4<f32>) -> vec4<f32> { return mod289_4(((x * 34.0) + 10.0) * x); }

fn snoise2(v: vec2<f32>) -> f32 {
    let C = vec4(0.211324865405187, 0.366025403784439, -0.577350269189626, 0.024390243902439);
    var i = floor(v + dot(v, C.yy));
    let x0 = v - i + dot(i, C.xx);
    let i1 = select(vec2(0.0, 1.0), vec2(1.0, 0.0), x0.x > x0.y);
    var x12 = x0.xyxy + C.xxzz;
    x12 = vec4(x12.xy - i1, x12.zw);
    i = mod289_2(i);
    let p = permute3(permute3(i.y + vec3(0.0, i1.y, 1.0)) + i.x + vec3(0.0, i1.x, 1.0));
    var m = max(0.5 - vec3(dot(x0, x0), dot(x12.xy, x12.xy), dot(x12.zw, x12.zw)), vec3(0.0));
    m = m * m;
    m = m * m;
    let x = 2.0 * fract(p * C.www) - 1.0;
    let h = abs(x) - 0.5;
    let ox = floor(x + 0.5);
    let a0 = x - ox;
    m *= 1.79284291400159 - 0.85373472095314 * (a0 * a0 + h * h);
    let g = vec3(a0.x * x0.x + h.x * x0.y, a0.y * x12.x + h.y * x12.y, a0.z * x12.z + h.z * x12.w);
    return 130.0 * dot(m, g);
}

fn snoise3(v: vec3<f32>) -> f32 {
    let C = vec2(1.0 / 6.0, 1.0 / 3.0);
    let D = vec4(0.0, 0.5, 1.0, 2.0);
    var i = floor(v + dot(v, C.yyy));
    let x0 = v - i + dot(i, C.xxx);
    let g = step(x0.yzx, x0.xyz);
    let l = 1.0 - g;
    let i1 = min(g, l.zxy);
    let i2 = max(g, l.zxy);
    let x1 = x0 - i1 + C.xxx;
    let x2 = x0 - i2 + C.yyy;
    let x3 = x0 - D.yyy;
    i = mod289_3(i);
    let p = permute4(permute4(permute4(i.z + vec4(0.0, i1.z, i2.z, 1.0)) + i.y + vec4(0.0, i1.y, i2.y, 1.0))
        + i.x + vec4(0.0, i1.x, i2.x, 1.0));
    let n_ = 0.142857142857;
    let ns = n_ * D.wyz - D.xzx;
    let j = p - 49.0 * floor(p * ns.z * ns.z);
    let x_ = floor(j * ns.z);
    let y_ = floor(j - 7.0 * x_);
    let x = x_ * ns.x + ns.yyyy;
    let y = y_ * ns.x + ns.yyyy;
    let h = 1.0 - abs(x) - abs(y);
    let b0 = vec4(x.xy, y.xy);
    let b1 = vec4(x.zw, y.zw);
    let s0 = floor(b0) * 2.0 + 1.0;
    let s1 = floor(b1) * 2.0 + 1.0;
    let sh = -step(h, vec4(0.0));
    let a0 = b0.xzyw + s0.xzyw * sh.xxyy;
    let a1 = b1.xzyw + s1.xzyw * sh.zzww;
    var p0 = vec3(a0.xy, h.x);
    var p1 = vec3(a0.zw, h.y);
    var p2 = vec3(a1.xy, h.z);
    var p3 = vec3(a1.zw, h.w);
    let norm = 1.79284291400159 - 0.85373472095314 * vec4(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3));
    p0 *= norm.x;
    p1 *= norm.y;
    p2 *= norm.z;
    p3 *= norm.w;
    var m = max(0.5 - vec4(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)), vec4(0.0));
    m = m * m;
    return 105.0 * dot(m * m, vec4(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)));
}

fn fbm2(p0: vec2<f32>, oct: i32) -> f32 {
    var p = p0;
    var s = 0.0;
    var a = 0.5;
    for (var i = 0; i < 8; i++) {
        if i >= oct { break; }
        s += a * snoise2(p);
        p = rot2(0.6) * p * 2.03 + 11.7;
        a *= 0.5;
    }
    return s;
}

fn fbm3(p0: vec3<f32>, oct: i32) -> f32 {
    var p = p0;
    var s = 0.0;
    var a = 0.5;
    for (var i = 0; i < 8; i++) {
        if i >= oct { break; }
        s += a * snoise3(p);
        p = p * 2.03 + 11.7;
        a *= 0.5;
    }
    return s;
}

// ---- ordered dither: deterministic 0..1 threshold for pixel p ----
var<private> BAYER4: array<u32, 16> = array<u32, 16>(0u, 8u, 2u, 10u, 12u, 4u, 14u, 6u, 3u, 11u, 1u, 9u, 15u, 7u, 13u, 5u);
var<private> BAYER8: array<u32, 64> = array<u32, 64>(
    0u, 32u, 8u, 40u, 2u, 34u, 10u, 42u,
    48u, 16u, 56u, 24u, 50u, 18u, 58u, 26u,
    12u, 44u, 4u, 36u, 14u, 46u, 6u, 38u,
    60u, 28u, 52u, 20u, 62u, 30u, 54u, 22u,
    3u, 35u, 11u, 43u, 1u, 33u, 9u, 41u,
    51u, 19u, 59u, 27u, 49u, 17u, 57u, 25u,
    15u, 47u, 7u, 39u, 13u, 45u, 5u, 37u,
    63u, 31u, 55u, 23u, 61u, 29u, 53u, 21u);

fn bayer4(p: vec2<f32>) -> f32 {
    let ip = vec2<u32>(p - 4.0 * floor(p / 4.0));
    return (f32(BAYER4[ip.y * 4u + ip.x]) + 0.5) / 16.0;
}

fn bayer8(p: vec2<f32>) -> f32 {
    let ip = vec2<u32>(p - 8.0 * floor(p / 8.0));
    return (f32(BAYER8[ip.y * 8u + ip.x]) + 0.5) / 64.0;
}

// ---- engraving (pve glsl/common.ts). Call from uniform control flow: they use fwidth. ----
// Parallel lines at integer u whose width grows with `darkness` (0 paper .. 1 ink); `du` is the
// derivative of u (pass it explicitly for a coordinate whose fwidth lies, e.g. a wrapping angle).
fn hatch_d(u: f32, darkness: f32, du: f32) -> f32 {
    let f = abs(fract(u) - 0.5);
    let half_w = 0.5 * sat(darkness);
    let aa = max(du, 1e-4);
    return 1.0 - smoothstep(half_w - aa, half_w + aa, 0.5 - f);
}

fn hatch(u: f32, darkness: f32) -> f32 { return hatch_d(u, darkness, fwidth(u)); }

// A thin always-on hairline plus crosshatch in deep shadows.
fn engrave(uv: vec2<f32>, darkness: f32, freq: f32, angle: f32) -> f32 {
    let r = rot2(angle) * uv * freq;
    let a = hatch(r.y, darkness * 1.1);
    let b = hatch((rot2(1.1) * uv * freq).y, sat(darkness * 2.0 - 1.1));
    return max(a, b);
}

