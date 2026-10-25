//! Probable Mirror Shards: the shattering glass of breaked_neuro's mirror plate (`MirrorGeom` /
//! `MirrorGlass.drawShards` in `_mirror.ts`). Jagged cracks run out of an impact point and cut the
//! image into wedges; `Shatter` (seconds after the hit, animate it) flies every wedge away from
//! the impact, turning and growing towards the camera, over a flat ground.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct MirrorShards {
    pipeline: wgpu::RenderPipeline,
}

const IMPACT_X: usize = 0;
const IMPACT_Y: usize = 1;
const COUNT: usize = 2;
const SEED: usize = 3;
const SHATTER: usize = 4;
const GROUND: usize = 5;
const GROUND_OPACITY: usize = 6;
const EDGE: usize = 7;
const EDGE_WIDTH: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    ground: [f32; 3],
    ground_opacity: f32,
    edge: [f32; 3],
    shatter: f32,
    size: [f32; 2],
    impact: [f32; 2],
    /// Canvas px per full-res px, for line widths.
    scale: f32,
    /// Canvas px per 1920 px of comp width: flight distances scale with the comp.
    flight_scale: f32,
    edge_width: f32,
    seed: i32,
    count: i32,
    _pad: [i32; 3],
}

impl Effect for MirrorShards {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Mirror Shards",
        match_name: "PROBABLE MirrorShards",
        category: "Probable",
        description: "Cracks the image into jagged wedges that fly away from an impact point.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        // Fractions of the frame; pve's impact is (0.56, 0.24).
        ParamDesc::float(1, "impact_x", "Impact X", 0.56, -1.0, 2.0, 1.0),
        ParamDesc::float(2, "impact_y", "Impact Y", 0.24, -1.0, 2.0, 1.0),
        ParamDesc::float(3, "count", "Cracks", 14.0, 3.0, 32.0, 32.0),
        ParamDesc::float(4, "seed", "Seed", 0.0, 0.0, 1000.0, 100.0),
        // Seconds after the hit: 0 is cracked but whole. Animate it.
        ParamDesc::float(5, "shatter", "Shatter (s)", 0.0, 0.0, 30.0, 3.0),
        // The substrate showing through the gaps; pve's blue.
        ParamDesc::color(6, "ground", "Ground", [0.165, 0.122, 1.0]),
        ParamDesc::float(7, "ground_opacity", "Ground Opacity", 1.0, 0.0, 1.0, 1.0),
        ParamDesc::color(8, "edge", "Edge", [0.933, 0.914, 0.875]),
        ParamDesc::float(9, "edge_width", "Edge Width (px)", 2.0, 0.0, 32.0, 8.0),
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "shards", include_str!("shaders/shards.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let size = [cx.size[0] as f32, cx.size[1] as f32];
        let u = gpu.uniform(&Uniform {
            ground: values.color_linear(GROUND),
            ground_opacity: values.float(GROUND_OPACITY),
            edge: values.color_linear(EDGE),
            shatter: values.float(SHATTER),
            size,
            impact: [values.float(IMPACT_X) * size[0], values.float(IMPACT_Y) * size[1]],
            scale: 0.5 * (cx.frame.scale[0] + cx.frame.scale[1]),
            flight_scale: size[0] / 1920.0,
            edge_width: values.float(EDGE_WIDTH),
            seed: values.float(SEED) as i32,
            count: values.float(COUNT).round() as i32,
            _pad: [0; 3],
        });
        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let bg =
            gpu.bind(&self.pipeline, &[u.as_entire_binding(), sampler, wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
