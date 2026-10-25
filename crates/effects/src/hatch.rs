//! Probable Hatch: pve's engraving hatch (`hatch` / `engrave` in glsl/common.ts) driven by the
//! image's tone: dark areas get thick lines, crosshatch in deep shadow.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct Hatch {
    pipeline: wgpu::RenderPipeline,
}

const STYLE: usize = 0;
const SPACING: usize = 1;
const ANGLE: usize = 2;
const CONTRAST: usize = 3;
const INK: usize = 4;
const PAPER: usize = 5;
const MIX: usize = 6;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    ink: [f32; 3],
    freq: f32,
    paper: [f32; 3],
    angle: f32,
    scale: [f32; 2],
    contrast: f32,
    mix_amount: f32,
    style: u32,
    _pad: [u32; 3],
}

impl Effect for Hatch {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Hatch",
        match_name: "PROBABLE Hatch",
        category: "Probable",
        description: "Engraving-style hatch lines whose width follows the image's tone.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        ParamDesc::choice(1, "style", "Style", &["Hatch", "Engrave"], 1),
        ParamDesc::float(2, "spacing", "Line Spacing (px)", 6.0, 1.0, 200.0, 24.0),
        ParamDesc::float(3, "angle", "Angle (rad)", 0.785, -6.3, 6.3, 3.15),
        ParamDesc::float(4, "contrast", "Contrast", 1.0, 0.0, 8.0, 3.0),
        // pve's default palette: ink #0A0A0B, bone #EEE9DF.
        ParamDesc::color(5, "ink", "Ink", [0.039, 0.039, 0.043]),
        ParamDesc::color(6, "paper", "Paper", [0.933, 0.914, 0.875]),
        ParamDesc::float(7, "mix", "Mix", 1.0, 0.0, 1.0, 1.0),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "hatch", include_str!("shaders/hatch.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let u = gpu.uniform(&Uniform {
            ink: values.color_linear(INK),
            freq: 1.0 / values.float(SPACING),
            paper: values.color_linear(PAPER),
            angle: values.float(ANGLE),
            scale: cx.frame.scale,
            contrast: values.float(CONTRAST),
            mix_amount: values.float(MIX),
            style: values.choice(STYLE),
            _pad: [0; 3],
        });
        let bg = gpu.bind(&self.pipeline, &[u.as_entire_binding(), wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
