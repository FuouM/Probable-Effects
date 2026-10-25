//! Probable Film Grain: pve's two-scale mid-tone-weighted grain (post.ts final pass).

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct FilmGrain {
    pipeline: wgpu::RenderPipeline,
}

const AMOUNT: usize = 0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    amount: f32,
    /// `fract(time * 13.37) * 1000` and `fract(time * 7.13) * 1000`: the fine and coarse seeds.
    seed_fine: f32,
    seed_coarse: f32,
    _pad: f32,
    /// Host downsample factor: grain lives on the full-resolution pixel grid.
    scale: [f32; 2],
    _pad2: [f32; 2],
}

impl Effect for FilmGrain {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Film Grain",
        match_name: "PROBABLE FilmGrain",
        category: "Probable",
        description: "Two-scale animated grain, strongest in the mid-tones, added in display (sRGB) space.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        // sRGB units: pve's default is 0.055, 0.04-0.1 is the useful range.
        ParamDesc::float(1, "amount", "Amount", 0.055, 0.0, 1.0, 0.2),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "grain", include_str!("shaders/grain.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        // f64 so the seed keeps its fraction late in a long timeline.
        let seed = |rate: f64| ((cx.frame.time * rate).fract() * 1000.0) as f32;
        let u = gpu.uniform(&Uniform {
            amount: values.float(AMOUNT),
            seed_fine: seed(13.37),
            seed_coarse: seed(7.13),
            _pad: 0.0,
            scale: cx.frame.scale,
            _pad2: [0.0; 2],
        });
        let bg = gpu.bind(&self.pipeline, &[u.as_entire_binding(), wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
