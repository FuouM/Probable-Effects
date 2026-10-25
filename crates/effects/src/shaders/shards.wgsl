// Mirror shards, after MirrorGeom / MirrorGlass.drawShards in breaked_neuro's _mirror.ts.
// Rays leave the impact point at jagged angles; wedge k lies between ray k and ray k+1. Each
// output pixel is inverse-mapped through every wedge's flight (translate away, turn about the
// impact, grow) and the wedges are painted in order, like the Canvas2D original.

struct U {
    ground: vec3<f32>,  // linear
    ground_opacity: f32,
    edge: vec3<f32>,    // linear
    shatter: f32,       // seconds after the hit
    size: vec2<f32>,    // canvas px
    impact: vec2<f32>,  // canvas px
    scale: f32,         // canvas px per full-res px
    flight_scale: f32,  // canvas px per 1920 px of comp width
    edge_width: f32,    // full-res px
    seed: i32,
    count: i32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var src: texture_2d<f32>;

const MAX_RAYS: i32 = 32;
const RAY_SEGMENTS: i32 = 7;

// pve's hashI.
fn hash_i(a: i32, b: i32, s: i32) -> f32 {
    var h = bitcast<u32>(a) * 374761393u + bitcast<u32>(b) * 668265263u + bitcast<u32>(s) * 2147483647u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    return f32(h ^ (h >> 16u)) / 4294967296.0;
}

fn ray_angle(k: i32) -> f32 {
    // Angle of ray k, kept in k's own slot so the rays stay sorted around the circle.
    let n = f32(u.count);
    return ((f32(k) + 0.2 + 0.6 * hash_i(k + u.seed * 97, 21, 0)) / n) * TAU;
}

// Angle of ray k at distance r px from the impact: its base angle plus a wobble that changes
// at 7 points out to 1.4 widths, like pve's polyline.
fn ray_angle_at(k: i32, r: f32) -> f32 {
    let step_px = 1.4 * u.size.x / f32(RAY_SEGMENTS);
    let t = clamp(r / step_px, 0.0, f32(RAY_SEGMENTS));
    let q = i32(floor(t));
    let wob = (hash_i(k + u.seed * 97, max(q, 1), 22) - 0.5) * 0.1;
    var wob_next = wob;
    if q >= 1 && q < RAY_SEGMENTS {
        wob_next = (hash_i(k + u.seed * 97, q + 1, 22) - 0.5) * 0.1;
    }
    return ray_angle(k) + mix(wob, wob_next, fract(t) * select(0.0, 1.0, q >= 1));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy - u.impact;
    var acc = vec4(u.ground * u.ground_opacity, u.ground_opacity);
    let edge_px = u.edge_width * u.scale;
    let a = u.shatter;

    for (var k = 0; k < MAX_RAYS; k++) {
        if k >= u.count { break; }
        let k1 = (k + 1) % u.count;
        let wrap1 = select(0.0, TAU, k1 == 0);
        let mid = 0.5 * (ray_angle(k) + ray_angle(k1) + wrap1);

        // Flight of this wedge: away from the impact, turned about it, grown towards the camera.
        let seed_k = k + u.seed * 97;
        let late = max(0.0, a - 0.3);
        let d = (a * 160.0 + a * a * (500.0 + 700.0 * hash_i(seed_k, 31, 0)) + late * late * 300.0) * u.flight_scale;
        let rot = (hash_i(seed_k, 32, 0) - 0.5) * a * 0.9;
        let grow = 1.0 + a * (0.3 + 0.5 * hash_i(seed_k, 33, 0));

        // Inverse: undo the translation, the turn and the growth.
        let q = rot2(-rot) * (p - vec2(cos(mid), sin(mid)) * d) / grow;
        let r = length(q);
        // Angle of q relative to the wedge's middle, in (-PI, PI].
        let delta = atan2(sin(atan2(q.y, q.x) - mid), cos(atan2(q.y, q.x) - mid));

        // Signed distance to the wedge boundary in output px (positive inside).
        let lo = delta - (ray_angle_at(k, r) - mid);
        let hi = (ray_angle_at(k1, r) + wrap1 - mid) - delta;
        let s = min(lo, hi) * r * grow;

        if s > 0.0 {
            let uv = (u.impact + q) / u.size;
            if all(uv >= vec2(0.0)) && all(uv <= vec2(1.0)) {
                let c = textureSampleLevel(src, samp, uv, 0.0);
                acc = c + acc * (1.0 - c.a);
            }
        }
        // Stroke along the crack: a ground-coloured halo with the edge colour on the line.
        if edge_px > 0.0 {
            let line = abs(s);
            let halo = 1.0 - smoothstep(1.5 * edge_px, 1.5 * edge_px + 1.0, line);
            let core = 1.0 - smoothstep(0.5 * edge_px, 0.5 * edge_px + 1.0, line);
            acc = mix(acc, vec4(u.ground, 1.0), halo);
            acc = mix(acc, vec4(u.edge, 1.0), core);
        }
    }
    return acc;
}
