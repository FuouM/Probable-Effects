//! Probable Chromatic Aberration: pve's radial red/blue split (post.ts final pass).

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct ChromaticAberration {
    pipeline: wgpu::RenderPipeline,
}

const AMOUNT: usize = 0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    /// Offset at unit `dc * r^2`, in uv units.
    offset: f32,
    aspect: f32,
    _pad: [f32; 2],
}

impl Effect for ChromaticAberration {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Chromatic Aberration",
        match_name: "PROBABLE ChromaticAberration",
        category: "Probable",
        description: "Lens-style radial colour fringing: red pushed out, blue pulled in, growing with distance from the centre.",
    };

    const PARAMS: &'static [ParamDesc] = &[ParamDesc::float(1, "amount", "Amount (px)", 1.2, -64.0, 64.0, 8.0)];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "aberration", include_str!("shaders/aberration.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        // pve: off = dc * r2 * ca / width * 4, `ca` in full-resolution px.
        let full_width = cx.size[0] as f32 / cx.frame.scale[0];
        let u = gpu.uniform(&Uniform {
            offset: values.float(AMOUNT) / full_width * 4.0,
            aspect: cx.size[0] as f32 / cx.size[1] as f32,
            _pad: [0.0; 2],
        });
        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let bg =
            gpu.bind(&self.pipeline, &[u.as_entire_binding(), sampler, wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
