// ASCII filter. Glyphs are 5x7 bitmaps of pve's ramp " .:-=+*#%@" (dark to light), one u32 per
// row (5 bits, MSB left), 7 rows per glyph.

struct U {
    ink: vec3<f32>,     // linear
    cell_w: f32,        // cell size in full-res px
    paper: vec3<f32>,   // linear
    cell_h: f32,
    scale: vec2<f32>,   // host downsample factor
    contrast: f32,
    colour: u32,        // 0 paper, 1 image colour
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var src: texture_2d<f32>;

var<private> GLYPHS: array<u32, 70> = array<u32, 70>(
    // ' '
    0u, 0u, 0u, 0u, 0u, 0u, 0u,
    // '.'
    0u, 0u, 0u, 0u, 0u, 12u, 12u,
    // ':'
    0u, 12u, 12u, 0u, 12u, 12u, 0u,
    // '-'
    0u, 0u, 0u, 31u, 0u, 0u, 0u,
    // '='
    0u, 0u, 31u, 0u, 31u, 0u, 0u,
    // '+'
    0u, 4u, 4u, 31u, 4u, 4u, 0u,
    // '*'
    0u, 21u, 14u, 31u, 14u, 21u, 0u,
    // '#'
    10u, 10u, 31u, 10u, 31u, 10u, 10u,
    // '%'
    25u, 26u, 2u, 4u, 8u, 11u, 19u,
    // '@'
    14u, 17u, 23u, 21u, 23u, 16u, 14u);

// Glyph coverage at `g` in 0..1 across the glyph box (5 x 7 bits).
fn glyph_bit(index: u32, g: vec2<f32>) -> f32 {
    if g.x < 0.0 || g.y < 0.0 || g.x >= 1.0 || g.y >= 1.0 { return 0.0; }
    let col = u32(g.x * 5.0);
    let row = u32(g.y * 7.0);
    return f32((GLYPHS[index * 7u + row] >> (4u - col)) & 1u);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let cell = vec2(u.cell_w, u.cell_h);
    let px = in.pos.xy / u.scale;           // full-res px, y down
    let id = floor(px / cell);
    let local = (px - id * cell) / cell;    // 0..1 inside the cell

    // Cell tone: 3x3 taps across the cell.
    let size = vec2<f32>(textureDimensions(src));
    var mean = vec4(0.0);
    for (var j = 0; j < 3; j++) {
        for (var i = 0; i < 3; i++) {
            let p = (id + (vec2(f32(i), f32(j)) + 0.5) / 3.0) * cell * u.scale;
            mean += textureSampleLevel(src, samp, p / size, 0.0);
        }
    }
    mean /= 9.0;
    let straight = unpremul(mean);
    let tone = sat((luma(sat3(to_srgb(straight))) - 0.5) * u.contrast + 0.5);
    let index = min(u32(tone * 10.0), 9u);

    // Glyph box: the cell minus a margin, supersampled 2x2 for soft edges.
    let margin = vec2(0.16, 0.12);
    let pixel_g = (1.0 / u.scale) / cell / (1.0 - 2.0 * margin);   // one canvas pixel in glyph units
    let g0 = (local - margin) / (1.0 - 2.0 * margin);
    var cover = 0.0;
    for (var j = 0; j < 2; j++) {
        for (var i = 0; i < 2; i++) {
            cover += glyph_bit(index, g0 + (vec2(f32(i), f32(j)) - 0.5) * 0.5 * pixel_g);
        }
    }
    cover *= 0.25;

    let glyph_col = select(u.paper, max(straight, vec3(0.0)), u.colour == 1u);
    return vec4(mix(u.ink, glyph_col, cover), 1.0);
}
