// Droste recursion. Coordinates are centre-relative canvas px (y down). Level k shows the whole
// frame scaled by ratio^k and turned by k * turn inside the window of level k - 1.

struct U {
    ring_a: vec3<f32>,  // linear
    ratio: f32,
    ring_b: vec3<f32>,
    turn: f32,
    ring_c: vec3<f32>,
    fall: f32,
    size: vec2<f32>,
    window: f32,        // window half-size, px
    ring_width: f32,    // fraction of the window
    ring_opacity: f32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var src: texture_2d<f32>;

const MAX_LEVELS: i32 = 48;

fn cheb(p: vec2<f32>) -> f32 { return max(abs(p.x), abs(p.y)); }

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let p = in.pos.xy - 0.5 * u.size;
    let B = u.window;
    let r = u.ratio;
    let into = rot2(-u.turn) * (1.0 / r);    // one level inward: turned, enlarged
    let outof = rot2(u.turn) * r;    // one level outward

    // Falling magnifies about the centre by r^-fall and turns by fall * turn.
    var q = rot2(-u.fall * u.turn) * p * pow(r, u.fall);
    var k = 0;
    if cheb(q) <= B {
        for (var i = 0; i < MAX_LEVELS; i++) {
            if cheb(q) > B { break; }
            q = into * q;
            k += 1;
        }
    } else {
        for (var i = 0; i < MAX_LEVELS; i++) {
            let o = outof * q;
            if cheb(o) <= B { break; }
            q = o;
            k -= 1;
        }
    }

    let c = textureSampleLevel(src, samp, (q + 0.5 * u.size) / u.size, 0.0);

    // Ring: the band of this level just outside its window, in turn ring 1, 2, 3.
    let phase = ((k % 3) + 3) % 3;
    var ring = u.ring_c;
    if phase == 0 { ring = u.ring_a; } else if phase == 1 { ring = u.ring_b; }
    let in_ring = select(0.0, u.ring_opacity, cheb(q) < B * (1.0 + u.ring_width));
    return vec4(mix(c.rgb, ring * c.a, in_ring), c.a);
}
