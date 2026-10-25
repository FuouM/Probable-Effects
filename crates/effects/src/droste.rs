//! Probable Droste: breaked_neuro's endless recursion (`_droste.ts`) as a filter. The image has
//! a square window at its centre showing a smaller copy of the whole image (pve: 0.44x, turned
//! 0.16 rad), which has its own window, and so on. `Fall` slides through the levels (animate it
//! to fall into the tunnel); the band just outside each window is filled in turn with three ring
//! colours (pve: blue, ink, paper).

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct Droste {
    pipeline: wgpu::RenderPipeline,
}

const WINDOW: usize = 0;
const RATIO: usize = 1;
const TURN: usize = 2;
const FALL: usize = 3;
const RING_WIDTH: usize = 4;
const RING_OPACITY: usize = 5;
const RING_A: usize = 6;
const RING_B: usize = 7;
const RING_C: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    ring_a: [f32; 3],
    ratio: f32,
    ring_b: [f32; 3],
    turn: f32,
    ring_c: [f32; 3],
    fall: f32,
    size: [f32; 2],
    /// Window half-size in canvas px.
    window: f32,
    ring_width: f32,
    ring_opacity: f32,
    _pad: [f32; 3],
}

impl Effect for Droste {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Droste",
        match_name: "PROBABLE Droste",
        category: "Probable",
        description: "Endless picture-in-picture recursion down the centre of the frame.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        // Fractions of the frame height; 0.22 is where the next level just fills the frame.
        ParamDesc::float(1, "window", "Window Half-Size", 0.22, 0.02, 1.0, 0.5),
        ParamDesc::float(2, "ratio", "Level Scale", 0.44, 0.05, 0.95, 0.9),
        ParamDesc::float(3, "turn", "Level Turn (rad)", 0.16, -3.15, 3.15, 1.0),
        // Levels fallen into; animate it. Negative climbs back out.
        ParamDesc::float(4, "fall", "Fall (levels)", 0.0, -64.0, 64.0, 8.0),
        ParamDesc::float(5, "ring_width", "Ring Width", 0.06, 0.0, 1.0, 0.3),
        ParamDesc::float(6, "ring_opacity", "Ring Opacity", 0.0, 0.0, 1.0, 1.0),
        // pve: blue, ink and paper in turn.
        ParamDesc::color(7, "ring_a", "Ring 1", [0.165, 0.122, 1.0]),
        ParamDesc::color(8, "ring_b", "Ring 2", [0.039, 0.039, 0.043]),
        ParamDesc::color(9, "ring_c", "Ring 3", [0.933, 0.914, 0.875]),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "droste", include_str!("shaders/droste.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let size = [cx.size[0] as f32, cx.size[1] as f32];
        let u = gpu.uniform(&Uniform {
            ring_a: values.color_linear(RING_A),
            ratio: values.float(RATIO),
            ring_b: values.color_linear(RING_B),
            turn: values.float(TURN),
            ring_c: values.color_linear(RING_C),
            fall: values.float(FALL),
            size,
            window: values.float(WINDOW) * size[1],
            ring_width: values.float(RING_WIDTH),
            ring_opacity: values.float(RING_OPACITY),
            _pad: [0.0; 3],
        });
        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let bg =
            gpu.bind(&self.pipeline, &[u.as_entire_binding(), sampler, wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
