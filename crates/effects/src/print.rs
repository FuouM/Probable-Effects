//! Probable Print: the binarising print pass from breaked_neuro's `PrintScene` (`_kit.ts`, the
//! `PRINT` shader) as a filter: alpha and tone become two halftone screens of ink and paper over
//! a flat ground, with wandering ink edges. The plate overlays of the original (Life plane,
//! scratches, clumps) belong to its scenes and are not part of this effect.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct Print {
    pipeline: wgpu::RenderPipeline,
}

const CELL: usize = 0;
const SOFT: usize = 1;
const BLEED: usize = 2;
const SEED: usize = 3;
const TEXTURE: usize = 4;
const INK: usize = 5;
const PAPER: usize = 6;
const GROUND: usize = 7;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    ink: [f32; 3],
    cell: f32,
    paper: [f32; 3],
    soft: f32,
    ground: [f32; 3],
    bleed: f32,
    scale: [f32; 2],
    size: [f32; 2],
    seed: f32,
    texture: f32,
    _pad: [f32; 2],
}

impl Effect for Print {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Print",
        match_name: "PROBABLE Print",
        category: "Probable",
        description: "Binarises the image into a halftone print: ink and paper dots over a flat ground.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        // pve: mix(4, 13, coarse).
        ParamDesc::float(1, "cell", "Halftone Cell (px)", 4.0, 1.0, 128.0, 13.0),
        ParamDesc::float(2, "soft", "Edge Softness", 0.06, 0.0, 0.5, 0.2),
        ParamDesc::float(3, "bleed", "Ink Bleed (px)", 0.0, 0.0, 32.0, 4.0),
        // pve re-seeds the wander at 12 fps; animate this (expression: Math.floor(time * 12)) to boil.
        ParamDesc::float(4, "seed", "Boil Seed", 0.0, 0.0, 1.0e6, 100.0),
        ParamDesc::float(5, "texture", "Paper Texture", 1.0, 0.0, 4.0, 2.0),
        ParamDesc::color(6, "ink", "Ink", [0.0, 0.0, 0.0]),
        ParamDesc::color(7, "paper", "Paper", [0.933, 0.914, 0.875]),
        // breaked_neuro's blue substrate showing through the voids.
        ParamDesc::color(8, "ground", "Ground", [0.165, 0.122, 1.0]),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "print", include_str!("shaders/print.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let u = gpu.uniform(&Uniform {
            ink: values.color_linear(INK),
            cell: values.float(CELL),
            paper: values.color_linear(PAPER),
            soft: values.float(SOFT),
            ground: values.color_linear(GROUND),
            bleed: values.float(BLEED),
            scale: cx.frame.scale,
            size: [cx.size[0] as f32, cx.size[1] as f32],
            seed: values.float(SEED),
            texture: values.float(TEXTURE),
            _pad: [0.0; 2],
        });
        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let bg =
            gpu.bind(&self.pipeline, &[u.as_entire_binding(), sampler, wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
