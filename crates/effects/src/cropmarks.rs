//! Probable Crop Marks: the corner marks of pve's HUD layer (`Hud.cropMarks` in hud.ts). As
//! `Frame` drops from 1 to 0 the marks fly out along the diagonals and past the edges.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct CropMarks {
    pipeline: wgpu::RenderPipeline,
}

const FRAME: usize = 0;
const OPACITY: usize = 1;
const COLOUR: usize = 2;
const ALPHA: usize = 3;
const LENGTH: usize = 4;
const WIDTH: usize = 5;
const INSET: usize = 6;
const FLY_OUT: usize = 7;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    colour: [f32; 3],
    /// Mark opacity: pve's colour alpha x HUD opacity x clamp(frame * 3).
    alpha: f32,
    size: [f32; 2],
    /// Mark position from the frame edge, canvas px.
    inset: f32,
    /// Arm length, canvas px.
    length: f32,
    /// Line half-width, canvas px.
    half_width: f32,
    _pad: [f32; 3],
}

/// pve's `ease.inOutCubic`.
fn in_out_cubic(t: f32) -> f32 {
    if t < 0.5 { 4.0 * t * t * t } else { 1.0 - (-2.0 * t + 2.0).powi(3) / 2.0 }
}

impl Effect for CropMarks {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Crop Marks",
        match_name: "PROBABLE CropMarks",
        category: "Probable",
        description: "Corner crop marks over the image; animate Frame to fly them in from past the edges.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        // 1 = in place, 0 = flown out past the edges.
        ParamDesc::float(1, "frame", "Frame", 1.0, 0.0, 1.0, 1.0),
        ParamDesc::float(2, "opacity", "Opacity", 1.0, 0.0, 1.0, 1.0),
        // pve: bone at 0.34 on dark plates, ink at 0.45 on paper plates.
        ParamDesc::color(3, "colour", "Mark Colour", [0.933, 0.914, 0.875]),
        ParamDesc::float(4, "alpha", "Mark Alpha", 0.34, 0.0, 1.0, 1.0),
        // The next three are px on a 1920-wide frame; they scale with the comp width.
        ParamDesc::float(5, "length", "Arm Length", 22.0, 1.0, 400.0, 100.0),
        ParamDesc::float(6, "width", "Line Width", 1.25, 0.1, 32.0, 8.0),
        ParamDesc::float(7, "inset", "Inset", 36.0, -400.0, 400.0, 120.0),
        ParamDesc::float(8, "fly_out", "Flown-Out Position", -40.0, -1000.0, 400.0, 120.0),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "cropmarks", include_str!("shaders/cropmarks.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let size = [cx.size[0] as f32, cx.size[1] as f32];
        let px = size[0] / 1920.0;
        let k = values.float(FRAME).clamp(0.0, 1.0);
        // pve: m = lerp(-40, 36, ease(k)); alpha *= clamp(k * 3).
        let from = values.float(FLY_OUT);
        let inset = from + (values.float(INSET) - from) * in_out_cubic(k);
        let u = gpu.uniform(&Uniform {
            colour: values.color_linear(COLOUR),
            alpha: values.float(ALPHA) * values.float(OPACITY) * (k * 3.0).clamp(0.0, 1.0),
            size,
            inset: inset * px,
            length: values.float(LENGTH) * px,
            half_width: 0.5 * values.float(WIDTH) * px,
            _pad: [0.0; 3],
        });
        let bg = gpu.bind(&self.pipeline, &[u.as_entire_binding(), wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
