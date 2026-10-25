//! A shader that does not compile must come back as an error naming the shader problem, every
//! time, and must not stop a working effect from rendering afterwards (the previewer edits
//! shaders live). Needs a GPU.

use probable_core::{
    ChannelOrder, ColorSpace, Depth, Effect, EffectInfo, Frame, Gpu, HostImage, HostImageMut, ImageDesc, ParamDesc,
    PixelLayout, Processor, RenderCx, Values, wgpu,
};

/// Copies the input canvas to the output; `BROKEN` swaps in a shader with an undefined name.
struct Copy<const BROKEN: bool> {
    pipeline: wgpu::RenderPipeline,
}

impl<const BROKEN: bool> Effect for Copy<BROKEN> {
    const INFO: EffectInfo = EffectInfo { name: "Copy", match_name: "test", category: "test", description: "" };
    const PARAMS: &'static [ParamDesc] = &[];

    fn new(gpu: &Gpu) -> Self {
        let value = if BROKEN { "undefined_name" } else { "textureLoad(src, vec2<i32>(in.pos.xy), 0)" };
        let module = gpu.fullscreen_module(
            "copy",
            &format!(
                "@group(0) @binding(0) var src: texture_2d<f32>;
                 @fragment fn fs(in: VsOut) -> @location(0) vec4<f32> {{ return {value}; }}"
            ),
        );
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, _: &Values) {
        let bg = cx.gpu.bind(&self.pipeline, &[wgpu::BindingResource::TextureView(cx.src)]);
        cx.gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}

fn render<E: Effect>(processor: &Processor<E>) -> Result<Vec<u8>, String> {
    let desc = ImageDesc {
        width: 8,
        height: 8,
        stride: 32,
        bottom_up: false,
        straight_alpha: true,
        layout: PixelLayout { order: ChannelOrder::Rgba, depth: Depth::U8 },
        origin: [0, 0],
    };
    let input: Vec<u8> =
        (0..8 * 8 * 4).map(|i| (i * 7 % 256) as u8).map(|v| if v % 4 == 3 { 255 } else { v }).collect();
    let mut out = vec![0u8; input.len()];
    let frame = Frame { time: 0.0, scale: [1.0; 2], space: ColorSpace::Srgb, grow: false };
    processor
        .render(
            &HostImage { data: &input, desc },
            &mut HostImageMut { data: &mut out, desc },
            &frame,
            &Values::defaults(&[]),
        )
        .map_err(|e| e.to_string())?;
    Ok(out)
}

#[test]
fn broken_shader_reports_its_error_each_time_and_does_not_poison_other_effects() {
    let broken = Processor::<Copy<true>>::default();
    for _ in 0..2 {
        let err = render(&broken).expect_err("a shader with an undefined name cannot render");
        assert!(err.contains("undefined_name"), "the error should name the shader problem, got: {err}");
    }

    let working = Processor::<Copy<false>>::default();
    let out = render(&working).expect("a working effect renders after a broken one failed");
    let expected: Vec<u8> =
        (0..8 * 8 * 4).map(|i| (i * 7 % 256) as u8).map(|v| if v % 4 == 3 { 255 } else { v }).collect();
    let worst = out.iter().zip(&expected).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 1, "copy should round-trip pixels, worst channel error {worst}");
}
