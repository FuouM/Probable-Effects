//! Probable Bloom: pve's bloom pyramid as a standalone filter.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, ParamKind, RenderCx, Values, wgpu};

use crate::pyramid::{self, Pyramid, Settings};

pub struct Bloom {
    pyramid: Pyramid,
    composite: wgpu::RenderPipeline,
}

const INTENSITY: usize = 0;
const THRESHOLD: usize = 1;
const KNEE: usize = 2;
const SPREAD: usize = 3;
const SIZE: usize = 4;
const OUTPUT: usize = 5;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    intensity: f32,
    mode: u32,
}

impl Effect for Bloom {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Bloom",
        match_name: "PROBABLE Bloom",
        category: "Probable",
        description: "Soft-threshold HDR bloom pyramid from probable-video-editor.",
    };

    const PARAMS: &'static [ParamDesc] = &[
        ParamDesc {
            id: 1,
            key: "intensity",
            label: "Intensity",
            kind: ParamKind::Float {
                default: 0.55,
                min: 0.0,
                max: 20.0,
                slider_min: 0.0,
                slider_max: 3.0,
                precision: 2,
            },
        },
        ParamDesc {
            id: 2,
            key: "threshold",
            label: "Threshold",
            kind: ParamKind::Float {
                default: 0.85,
                min: 0.0,
                max: 100.0,
                slider_min: 0.0,
                slider_max: 2.0,
                precision: 2,
            },
        },
        ParamDesc {
            id: 3,
            key: "knee",
            label: "Knee",
            kind: ParamKind::Float {
                default: 0.5,
                min: 0.0,
                max: 10.0,
                slider_min: 0.0,
                slider_max: 1.0,
                precision: 2,
            },
        },
        ParamDesc {
            id: 4,
            key: "spread",
            label: "Spread",
            kind: ParamKind::Float {
                default: 0.75,
                min: 0.0,
                max: 4.0,
                slider_min: 0.0,
                slider_max: 1.0,
                precision: 2,
            },
        },
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
        ParamDesc {
            id: 6,
            key: "output",
            label: "Output",
            kind: ParamKind::Choice { options: &["Composite", "Bloom Only"], default: 0 },
        },
    ];

    const CAN_GROW: bool = true;

    fn new(gpu: &Gpu) -> Self {
        let module = gpu.fullscreen_module("bloom", include_str!("shaders/bloom.wgsl"));
        Self {
            pyramid: Pyramid::new(gpu),
            composite: gpu.fullscreen_pipeline(&module, "fs_composite", wgpu::TextureFormat::Rgba32Float),
        }
    }

    fn margin(values: &Values, scale: [f32; 2]) -> [u32; 2] {
        [pyramid::reach(values.float(SIZE), scale, 0.5 + values.float(SPREAD)); 2]
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let bloom = self.pyramid.build(
            cx,
            &Settings {
                threshold: values.float(THRESHOLD),
                knee: values.float(KNEE),
                radius: 0.5 + values.float(SPREAD),
                size_px: values.float(SIZE),
            },
        );

        let gpu = cx.gpu;
        let u = gpu.uniform(&Uniform {
            // The pyramid sums ~7 levels; pve normalises by 3.
            intensity: values.float(INTENSITY) / 3.0,
            mode: values.choice(OUTPUT),
        });
        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let bg = gpu.bind(
            &self.composite,
            &[
                u.as_entire_binding(),
                sampler,
                wgpu::BindingResource::TextureView(bloom.level(0)),
                wgpu::BindingResource::TextureView(cx.src),
            ],
        );
        gpu.draw(cx.encoder, &self.composite, &bg, cx.dst);
    }
}
