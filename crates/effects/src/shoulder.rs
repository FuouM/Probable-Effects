//! Probable Tone Shoulder: pve's highlight roll-off (`SHOULDER_GLSL` in post.ts) as a filter.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct Shoulder {
    pipeline: wgpu::RenderPipeline,
}

const EXPOSURE: usize = 0;
const START: usize = 1;
const WHITE: usize = 2;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    exposure: f32,
    start: f32,
    white: f32,
    _pad: f32,
}

impl Effect for Shoulder {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Tone Shoulder",
        match_name: "PROBABLE Shoulder",
        category: "Probable",
        description: "Identity below the knee, smooth exponential shoulder above it; very bright values roll to white.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        // pve multiplies the frame by `exposure` just before the shoulder.
        ParamDesc::float(1, "exposure", "Exposure", 1.0, 0.0, 64.0, 4.0),
        ParamDesc::float(2, "start", "Shoulder Start", 0.72, 0.0, 0.99, 0.99),
        ParamDesc::float(3, "white", "Roll to White", 0.85, 0.0, 1.0, 1.0),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "shoulder", include_str!("shaders/shoulder.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let u = gpu.uniform(&Uniform {
            exposure: values.float(EXPOSURE),
            start: values.float(START),
            white: values.float(WHITE),
            _pad: 0.0,
        });
        let bg = gpu.bind(&self.pipeline, &[u.as_entire_binding(), wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
