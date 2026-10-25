// Host pixels <-> canvas (linear Rec.709 primaries, premultiplied, rgba32float).
//
// unpack_*: one invocation per canvas pixel; reads the host texture at (canvas - offset).
// pack_*:   one invocation per host pixel;   reads the canvas at (host + offset).

struct Io {
    offset: vec2<i32>,
    size: vec2<u32>,
    order: u32,       // 0 ARGB, 1 BGRA, 2 RGBA
    int_scale: f32,   // integer texel -> 0..1 divisor (u16 formats)
    transfer: u32,    // 0 sRGB, 1 linear, 2 gamma 2.4, 3 ACEScct
    bottom_up: u32,   // host memory row 0 is the bottom image row
    straight: u32,    // host alpha is straight (unpremultiplied)
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    m0: vec4<f32>,    // gamut matrix rows (to Rec.709 for unpack, from Rec.709 for pack)
    m1: vec4<f32>,
    m2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> io: Io;
@group(0) @binding(1) var host_f: texture_2d<f32>;
@group(0) @binding(2) var host_u: texture_2d<u32>;
@group(0) @binding(3) var canvas_w: texture_storage_2d<rgba32float, write>;
@group(0) @binding(4) var canvas_r: texture_2d<f32>;
@group(0) @binding(5) var out_f32: texture_storage_2d<rgba32float, write>;
@group(0) @binding(6) var out_u8: texture_storage_2d<rgba8unorm, write>;
@group(0) @binding(7) var out_u16: texture_storage_2d<rgba16uint, write>;

// Host channel order -> RGBA.
fn to_rgba(t: vec4<f32>) -> vec4<f32> {
    switch io.order {
        case 0u: { return t.yzwx; }
        case 1u: { return t.zyxw; }
        default: { return t; }
    }
}

// RGBA -> host channel order.
fn from_rgba(c: vec4<f32>) -> vec4<f32> {
    switch io.order {
        case 0u: { return c.wxyz; }
        case 1u: { return c.zyxw; }
        default: { return c; }
    }
}

fn srgb_to_linear(v: f32) -> f32 {
    let a = abs(v);
    let l = select(pow((a + 0.055) / 1.055, 2.4), a / 12.92, a <= 0.04045);
    return sign(v) * l;
}

fn linear_to_srgb(v: f32) -> f32 {
    let a = abs(v);
    let e = select(1.055 * pow(a, 1.0 / 2.4) - 0.055, a * 12.92, a <= 0.0031308);
    return sign(v) * e;
}

fn acescct_to_linear(v: f32) -> f32 {
    if v <= 0.155251141552511 {
        return (v - 0.0729055341958355) / 10.5402377416545;
    }
    return exp2(v * 17.52 - 9.72);
}

fn linear_to_acescct(v: f32) -> f32 {
    if v <= 0.0078125 {
        return 10.5402377416545 * v + 0.0729055341958355;
    }
    return (log2(v) + 9.72) / 17.52;
}

fn decode(v: f32) -> f32 {
    switch io.transfer {
        case 0u: { return srgb_to_linear(v); }
        case 2u: { return sign(v) * pow(abs(v), 2.4); }
        case 3u: { return acescct_to_linear(v); }
        default: { return v; }
    }
}

fn encode(v: f32) -> f32 {
    switch io.transfer {
        case 0u: { return linear_to_srgb(v); }
        case 2u: { return sign(v) * pow(abs(v), 1.0 / 2.4); }
        case 3u: { return linear_to_acescct(v); }
        default: { return v; }
    }
}

fn gamut(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(dot(io.m0.xyz, c), dot(io.m1.xyz, c), dot(io.m2.xyz, c));
}

// Transfer functions apply to straight colour: unpremultiply around them. The canvas is always
// premultiplied; straight hosts are premultiplied on the way in and unpremultiplied on the way out.
// A linear transfer commutes with premultiplication, so premultiplied linear hosts skip the
// divide/multiply and round-trip exactly.
fn decode_px(p: vec4<f32>) -> vec4<f32> {
    let a = p.a;
    let straight = io.straight != 0u;
    if io.transfer == 1u {
        return vec4<f32>(gamut(p.rgb) * select(1.0, a, straight), a);
    }
    let k = select(1.0, a, a > 0.0);
    let s = select(p.rgb / k, p.rgb, straight);
    let lin = gamut(vec3<f32>(decode(s.r), decode(s.g), decode(s.b)));
    return vec4<f32>(lin * select(k, a, straight), a);
}

fn encode_px(p: vec4<f32>) -> vec4<f32> {
    let a = p.a;
    let straight = io.straight != 0u;
    let k = select(1.0, a, a > 0.0);
    if io.transfer == 1u {
        return vec4<f32>(gamut(select(p.rgb, p.rgb / k, straight)), a);
    }
    let s = gamut(p.rgb / k);
    let e = vec3<f32>(encode(s.r), encode(s.g), encode(s.b));
    return vec4<f32>(select(e * k, e, straight), a);
}

fn host_row(y: i32, height: i32) -> i32 {
    return select(y, height - 1 - y, io.bottom_up != 0u);
}

// Host texel for canvas pixel `id`, or -1 when outside the host image.
fn host_coord(id: vec2<u32>, dims: vec2<u32>) -> vec2<i32> {
    let p = vec2<i32>(id) - io.offset;
    let d = vec2<i32>(dims);
    if any(p < vec2<i32>(0)) || any(p >= d) {
        return vec2<i32>(-1);
    }
    return vec2<i32>(p.x, host_row(p.y, d.y));
}

@compute @workgroup_size(16, 16)
fn unpack_float(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= io.size) { return; }
    let h = host_coord(id.xy, textureDimensions(host_f));
    var c = vec4<f32>(0.0);
    if h.x >= 0 {
        c = decode_px(to_rgba(textureLoad(host_f, h, 0)));
    }
    textureStore(canvas_w, id.xy, c);
}

@compute @workgroup_size(16, 16)
fn unpack_uint(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= io.size) { return; }
    let h = host_coord(id.xy, textureDimensions(host_u));
    var c = vec4<f32>(0.0);
    if h.x >= 0 {
        c = decode_px(to_rgba(vec4<f32>(textureLoad(host_u, h, 0)) / io.int_scale));
    }
    textureStore(canvas_w, id.xy, c);
}

// Canvas pixel for host texel `id` (texel rows are in memory order).
fn canvas_px(id: vec2<u32>) -> vec4<f32> {
    let y = host_row(i32(id.y), i32(io.size.y));
    return encode_px(textureLoad(canvas_r, vec2<i32>(i32(id.x), y) + io.offset, 0));
}

@compute @workgroup_size(16, 16)
fn pack_f32(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= io.size) { return; }
    textureStore(out_f32, id.xy, from_rgba(canvas_px(id.xy)));
}

@compute @workgroup_size(16, 16)
fn pack_u8(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= io.size) { return; }
    textureStore(out_u8, id.xy, from_rgba(clamp(canvas_px(id.xy), vec4<f32>(0.0), vec4<f32>(1.0))));
}

@compute @workgroup_size(16, 16)
fn pack_u16(@builtin(global_invocation_id) id: vec3<u32>) {
    if any(id.xy >= io.size) { return; }
    let c = from_rgba(clamp(canvas_px(id.xy), vec4<f32>(0.0), vec4<f32>(1.0)));
    textureStore(out_u16, id.xy, vec4<u32>(round(c * io.int_scale)));
}
