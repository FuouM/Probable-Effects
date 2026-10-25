// Palette: the image passes through; "Show Swatches" draws the nine colours as a strip along the
// bottom edge so the palette can be checked.

struct U {
    colours: array<vec4<f32>, 9>,   // linear
    swatches: u32,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let base = textureLoad(src, vec2<i32>(in.pos.xy), 0);
    if u.swatches == 0u || in.uv.y < 0.9 {
        return base;
    }
    let i = min(u32(in.uv.x * 9.0), 8u);
    return vec4(u.colours[i].rgb, 1.0);
}
