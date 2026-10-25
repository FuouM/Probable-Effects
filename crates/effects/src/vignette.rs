//! Probable Vignette: pve's final-pass darkening toward the frame edge (post.ts).

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct Vignette {
    pipeline: wgpu::RenderPipeline,
}

const AMOUNT: usize = 0;
const INNER: usize = 1;
const OUTER: usize = 2;
const SQUEEZE: usize = 3;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    amount: f32,
    inner: f32,
    outer: f32,
    squeeze: f32,
}

impl Effect for Vignette {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Vignette",
        match_name: "PROBABLE Vignette",
        category: "Probable",
        description: "Smooth darkening toward the frame edge.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        ParamDesc::float(1, "amount", "Amount", 0.35, 0.0, 1.0, 1.0),
        // Distances from the frame centre in frame heights/widths (0.5 = the edge midpoint).
        ParamDesc::float(2, "inner", "Inner Radius", 0.25, 0.0, 2.0, 1.0),
        ParamDesc::float(3, "outer", "Outer Radius", 0.95, 0.0, 2.0, 1.0),
        // pve measures the vertical axis at 0.8: the falloff is slightly wider than tall.
        ParamDesc::float(4, "squeeze", "Vertical Scale", 0.8, 0.1, 2.0, 1.5),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "vignette", include_str!("shaders/vignette.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let u = gpu.uniform(&Uniform {
            amount: values.float(AMOUNT),
            inner: values.float(INNER),
            outer: values.float(OUTER),
            squeeze: values.float(SQUEEZE),
        });
        let bg = gpu.bind(&self.pipeline, &[u.as_entire_binding(), wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
