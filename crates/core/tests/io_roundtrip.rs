//! Host pixels must survive unpack → canvas → pack unchanged for every layout, colour space and
//! alpha mode, or every effect would shift colours in the host. Needs a GPU.

use probable_core::{
    ChannelOrder, ColorSpace, Depth, Effect, EffectInfo, Frame, Gpu, HostImage, HostImageMut, ImageDesc, ParamDesc,
    PixelLayout, Processor, RenderCx, Values, wgpu,
};

/// Copies the input canvas to the output canvas.
struct Passthrough {
    pipeline: wgpu::RenderPipeline,
}

impl Effect for Passthrough {
    const INFO: EffectInfo = EffectInfo { name: "Passthrough", match_name: "test", category: "test", description: "" };
    const PARAMS: &'static [ParamDesc] = &[];

    fn new(gpu: &Gpu) -> Self {
        let module = gpu.fullscreen_module(
            "passthrough",
            "@group(0) @binding(0) var src: texture_2d<f32>;
             @fragment fn fs(in: VsOut) -> @location(0) vec4<f32> { return textureLoad(src, vec2<i32>(in.pos.xy), 0); }",
        );
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, _: &Values) {
        let bg = cx.gpu.bind(&self.pipeline, &[wgpu::BindingResource::TextureView(cx.src)]);
        cx.gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}

const W: u32 = 37;
const H: u32 = 23;

/// Deterministic test pattern in 0..1, alpha including 0 and 1, colour premultiplied when asked.
/// Fully transparent pixels carry no colour: the premultiplied canvas cannot keep it.
fn pattern(premultiplied: bool) -> Vec<[f32; 4]> {
    (0..W * H)
        .map(|i| {
            let h = |k: u32| ((i.wrapping_mul(2654435761).wrapping_add(k * 40503)) % 1000) as f32 / 999.0;
            let a = match i % 5 {
                0 => 1.0,
                1 => 0.0,
                _ => h(3),
            };
            let k = if premultiplied || a == 0.0 { a } else { 1.0 };
            [h(0) * k, h(1) * k, h(2) * k, a]
        })
        .collect()
}

fn encode(px: &[[f32; 4]], layout: PixelLayout, stride: usize) -> Vec<u8> {
    let bpp = layout.depth.bytes_per_pixel();
    let mut out = vec![0u8; stride * H as usize];
    for (i, p) in px.iter().enumerate() {
        let (x, y) = (i % W as usize, i / W as usize);
        let c = match layout.order {
            ChannelOrder::Argb => [p[3], p[0], p[1], p[2]],
            ChannelOrder::Bgra => [p[2], p[1], p[0], p[3]],
            ChannelOrder::Rgba => *p,
        };
        let at = y * stride + x * bpp;
        for (ch, v) in c.iter().enumerate() {
            match layout.depth {
                Depth::U8 => out[at + ch] = (v * 255.0).round() as u8,
                Depth::U16Ae | Depth::U16 => {
                    let max = if layout.depth == Depth::U16Ae { 32768.0 } else { 65535.0 };
                    out[at + ch * 2..at + ch * 2 + 2].copy_from_slice(&((v * max).round() as u16).to_ne_bytes());
                }
                Depth::F32 => out[at + ch * 4..at + ch * 4 + 4].copy_from_slice(&v.to_ne_bytes()),
            }
        }
    }
    out
}

/// Host-unit channel values of pixel `i`, in host channel order.
fn channels(buf: &[u8], layout: PixelLayout, stride: usize, i: usize) -> [f64; 4] {
    let (x, y) = (i % W as usize, i / W as usize);
    let px = &buf[y * stride + x * layout.depth.bytes_per_pixel()..];
    std::array::from_fn(|c| match layout.depth {
        Depth::U8 => px[c] as f64,
        Depth::U16Ae | Depth::U16 => u16::from_ne_bytes([px[c * 2], px[c * 2 + 1]]) as f64,
        Depth::F32 => f32::from_ne_bytes(px[c * 4..c * 4 + 4].try_into().unwrap()) as f64,
    })
}

/// Largest per-channel difference, in host units. Straight-alpha pixels with zero alpha are
/// compared on alpha only: their colour is undefined once premultiplied.
fn max_diff(a: &[u8], b: &[u8], layout: PixelLayout, stride: usize, straight: bool) -> f64 {
    let alpha = match layout.order {
        ChannelOrder::Argb => 0,
        ChannelOrder::Bgra | ChannelOrder::Rgba => 3,
    };
    let mut worst = 0.0f64;
    for i in 0..(W * H) as usize {
        let (pa, pb) = (channels(a, layout, stride, i), channels(b, layout, stride, i));
        for c in 0..4 {
            if straight && pa[alpha] == 0.0 && c != alpha {
                continue;
            }
            worst = worst.max((pa[c] - pb[c]).abs());
        }
    }
    worst
}

#[test]
fn every_layout_space_and_alpha_mode_round_trips() {
    let processor = Processor::<Passthrough>::default();
    let values = Values::defaults(&[]);
    let mut failures = Vec::new();
    for depth in [Depth::U8, Depth::U16Ae, Depth::U16, Depth::F32] {
        for order in [ChannelOrder::Argb, ChannelOrder::Bgra, ChannelOrder::Rgba] {
            for space in ColorSpace::ALL {
                for straight_alpha in [false, true] {
                    for bottom_up in [false, true] {
                        let layout = PixelLayout { order, depth };
                        // Padded rows, as hosts allocate them.
                        let stride = (W as usize * depth.bytes_per_pixel()).next_multiple_of(64) + 64;
                        let desc = ImageDesc {
                            width: W,
                            height: H,
                            stride,
                            bottom_up,
                            straight_alpha,
                            layout,
                            origin: [5, -3],
                        };
                        let input = encode(&pattern(!straight_alpha), layout, stride);
                        let mut output = vec![0u8; input.len()];
                        let frame = Frame { time: 0.0, scale: [1.0; 2], space, grow: false };
                        processor
                            .render(
                                &HostImage { data: &input, desc },
                                &mut HostImageMut { data: &mut output, desc },
                                &frame,
                                &values,
                            )
                            .unwrap();
                        // Integer formats: one code value (premultiply/unpremultiply rounding).
                        // Float: transfer-function precision. Rec.2020 is a pure power curve
                        // (infinite slope at 0) behind a channel-mixing matrix: f32 matrix
                        // round-off on a zero channel encodes to ~5e-4 of full scale.
                        let full: f64 = match depth {
                            Depth::U8 => 255.0,
                            Depth::U16Ae => 32768.0,
                            Depth::U16 => 65535.0,
                            Depth::F32 => 1.0,
                        };
                        let tolerance = match (depth, space) {
                            (_, ColorSpace::Rec2020) => (6e-4 * full).max(if depth == Depth::F32 { 0.0 } else { 1.0 }),
                            (Depth::F32, _) => 2e-4,
                            _ => 1.0,
                        };
                        let d = max_diff(&input, &output, layout, stride, straight_alpha);
                        if d > tolerance {
                            failures.push(format!(
                                "{depth:?} {order:?} {space:?} straight={straight_alpha} bottom_up={bottom_up}: {d}"
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "round-trip errors:\n{}", failures.join("\n"));
}

#[test]
fn output_larger_than_input_is_transparent_outside_and_aligned_inside() {
    let processor = Processor::<Passthrough>::default();
    let layout = PixelLayout { order: ChannelOrder::Argb, depth: Depth::F32 };
    let stride = W as usize * 16;
    let src_desc =
        ImageDesc { width: W, height: H, stride, bottom_up: false, straight_alpha: false, layout, origin: [0, 0] };
    let m = 7u32;
    let (dw, dh) = (W + 2 * m, H + 2 * m);
    let dst_desc = ImageDesc { width: dw, height: dh, stride: dw as usize * 16, origin: [-(m as i32); 2], ..src_desc };
    let input = encode(&pattern(true), layout, stride);
    let mut output = vec![0u8; dst_desc.stride * dh as usize];
    let frame = Frame { time: 0.0, scale: [1.0; 2], space: ColorSpace::Linear, grow: true };
    processor
        .render(
            &HostImage { data: &input, desc: src_desc },
            &mut HostImageMut { data: &mut output, desc: dst_desc },
            &frame,
            &Values::defaults(&[]),
        )
        .unwrap();
    for y in 0..dh as usize {
        for x in 0..dw as usize {
            let got = &output[y * dst_desc.stride + x * 16..][..16];
            let (sx, sy) = (x as i64 - m as i64, y as i64 - m as i64);
            if (0..W as i64).contains(&sx) && (0..H as i64).contains(&sy) {
                let want = &input[sy as usize * stride + sx as usize * 16..][..16];
                assert_eq!(got, want, "pixel ({x},{y})");
            } else {
                assert!(got.iter().all(|b| *b == 0), "pixel ({x},{y}) outside the input is not transparent");
            }
        }
    }
}
