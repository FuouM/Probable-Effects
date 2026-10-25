//! Probable ASCII: breaked_neuro's ASCII fabric (`_ascii.ts`) as a filter. The frame becomes a
//! grid of monospace cells (pve: 12x18 px); each cell's tone picks a glyph of the ramp
//! ` .:-=+*#%@` drawn in paper on ink.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct Ascii {
    pipeline: wgpu::RenderPipeline,
}

const CELL: usize = 0;
const ASPECT: usize = 1;
const CONTRAST: usize = 2;
const COLOUR: usize = 3;
const INK: usize = 4;
const PAPER: usize = 5;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    ink: [f32; 3],
    cell_w: f32,
    paper: [f32; 3],
    cell_h: f32,
    scale: [f32; 2],
    contrast: f32,
    colour: u32,
}

impl Effect for Ascii {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable ASCII",
        match_name: "PROBABLE Ascii",
        category: "Probable",
        description: "Redraws the image as a grid of monospace characters chosen by tone.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        // pve's grid: 12 x 18 px cells.
        ParamDesc::float(1, "cell", "Cell Width (px)", 12.0, 4.0, 128.0, 48.0),
        ParamDesc::float(2, "aspect", "Cell Height / Width", 1.5, 0.5, 4.0, 3.0),
        ParamDesc::float(3, "contrast", "Contrast", 1.0, 0.0, 8.0, 3.0),
        ParamDesc::choice(4, "colour", "Glyph Colour", &["Paper", "Image"], 0),
        ParamDesc::color(5, "ink", "Ink", [0.039, 0.039, 0.043]),
        ParamDesc::color(6, "paper", "Paper", [0.933, 0.914, 0.875]),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "ascii", include_str!("shaders/ascii.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let cell = values.float(CELL);
        let u = gpu.uniform(&Uniform {
            ink: values.color_linear(INK),
            cell_w: cell,
            paper: values.color_linear(PAPER),
            cell_h: cell * values.float(ASPECT),
            scale: cx.frame.scale,
            contrast: values.float(CONTRAST),
            colour: values.choice(COLOUR),
        });
        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let bg =
            gpu.bind(&self.pipeline, &[u.as_entire_binding(), sampler, wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
