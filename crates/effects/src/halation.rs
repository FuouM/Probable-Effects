//! Probable Halation: pve's red-orange film halation, a wide tinted glow around highlights from
//! the coarse end of the bloom pyramid (post.ts: `ups[3]`, luma-tinted by `vec3(1, 0.18, 0.04)`).

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, ParamKind, RenderCx, Values, wgpu};

use crate::pyramid::{self, Pyramid, Settings};

pub struct Halation {
    pyramid: Pyramid,
    composite: wgpu::RenderPipeline,
}

const INTENSITY: usize = 0;
const THRESHOLD: usize = 1;
const KNEE: usize = 2;
const SPREAD: usize = 3;
const SIZE: usize = 4;
const TINT: usize = 5;
const OUTPUT: usize = 6;

/// pve's halo is level 3 of a 7-level pyramid: the four coarsest levels' light.
const HALO_LEVELS: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    tint: [f32; 3],
    intensity: f32,
    mode: u32,
    _pad: [u32; 3],
}

impl Effect for Halation {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Halation",
        match_name: "PROBABLE Halation",
        category: "Probable",
        description: "Red-orange film halation: a wide tinted glow around highlights.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        ParamDesc::float(1, "intensity", "Intensity", 0.25, 0.0, 20.0, 2.0),
        ParamDesc::float(2, "threshold", "Threshold", 0.85, 0.0, 100.0, 2.0),
        ParamDesc::float(3, "knee", "Knee", 0.5, 0.0, 10.0, 1.0),
        ParamDesc::float(4, "spread", "Spread", 0.75, 0.0, 4.0, 1.0),
        ParamDesc {
            id: 5,
            key: "size",
            label: "Size (px)",
            kind: ParamKind::Float {
                default: 128.0,
                min: 2.0,
                max: 4096.0,
                slider_min: 8.0,
                slider_max: 512.0,
                precision: 0,
            },
        },
        // pve's linear (1, 0.18, 0.04), as the sRGB colour a picker shows.
        ParamDesc::color(6, "tint", "Tint", [1.0, 0.4615, 0.2208]),
        ParamDesc::choice(7, "output", "Output", &["Composite", "Halation Only"], 0),
    ];

    const CAN_GROW: bool = true;

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "halation", include_str!("shaders/halation.wgsl"));
        Self {
            pyramid: Pyramid::new(gpu),
            composite: gpu.fullscreen_pipeline(&module, "fs_composite", wgpu::TextureFormat::Rgba32Float),
        }
    }

    fn margin(values: &Values, scale: [f32; 2]) -> [u32; 2] {
        [pyramid::reach(values.float(SIZE), scale, 0.5 + values.float(SPREAD)); 2]
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let pyramid = self.pyramid.build(
            cx,
            &Settings {
                threshold: values.float(THRESHOLD),
                knee: values.float(KNEE),
                radius: 0.5 + values.float(SPREAD),
                size_px: values.float(SIZE),
            },
        );
        let halo = pyramid.level(pyramid.len().saturating_sub(HALO_LEVELS));

        let gpu = cx.gpu;
        let u = gpu.uniform(&Uniform {
            tint: values.color_linear(TINT),
            intensity: values.float(INTENSITY),
            mode: values.choice(OUTPUT),
            _pad: [0; 3],
        });
        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let bg = gpu.bind(
            &self.composite,
            &[
                u.as_entire_binding(),
                sampler,
                wgpu::BindingResource::TextureView(halo),
                wgpu::BindingResource::TextureView(cx.src),
            ],
        );
        gpu.draw(cx.encoder, &self.composite, &bg, cx.dst);
    }
}
