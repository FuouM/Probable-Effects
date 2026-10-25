//! Probable Bloom: pve's bloom pyramid as a standalone filter.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, ParamKind, RenderCx, Values, wgpu};

pub struct Bloom {
    prefilter: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
}

const INTENSITY: usize = 0;
const THRESHOLD: usize = 1;
const KNEE: usize = 2;
const SPREAD: usize = 3;
const SIZE: usize = 4;
const OUTPUT: usize = 5;

/// Pyramid texture format (pve uses HalfFloat targets).
const MIP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const MAX_LEVELS: u32 = 10;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PassUniform {
    texel: [f32; 2],
    threshold: f32,
    knee: f32,
    radius: f32,
    intensity: f32,
    mode: u32,
    _pad: u32,
}

/// Pyramid depth for a bloom `size` (full-res px) at host `scale`: the coarsest level's texel is
/// about `size` full-res px, as pve's 7 levels from 1920x1080 reach 128 px.
fn levels(values: &Values, scale: [f32; 2]) -> u32 {
    let px = values.float(SIZE) * 0.5 * (scale[0] + scale[1]);
    (px.max(2.0).log2().round() as u32).clamp(1, MAX_LEVELS)
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
            prefilter: gpu.fullscreen_pipeline(&module, "fs_prefilter", MIP_FORMAT),
            down: gpu.fullscreen_pipeline(&module, "fs_down", MIP_FORMAT),
            up: gpu.fullscreen_pipeline(&module, "fs_up", MIP_FORMAT),
            composite: gpu.fullscreen_pipeline(&module, "fs_composite", wgpu::TextureFormat::Rgba32Float),
        }
    }

    fn margin(values: &Values, scale: [f32; 2]) -> [u32; 2] {
        // The tent/13-tap chain spreads light about two coarsest texels past a bright edge.
        let reach = 2.0 * (1u32 << levels(values, scale)) as f32 * (0.5 + values.float(SPREAD));
        [reach.ceil() as u32; 2]
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let base = PassUniform {
            texel: [0.0; 2],
            threshold: values.float(THRESHOLD),
            knee: values.float(KNEE),
            radius: 0.5 + values.float(SPREAD),
            // The pyramid sums ~7 levels; pve normalises by 3.
            intensity: values.float(INTENSITY) / 3.0,
            mode: values.choice(OUTPUT),
            _pad: 0,
        };
        let texel = |size: [u32; 2]| [1.0 / size[0] as f32, 1.0 / size[1] as f32];

        // Stop before a level would shrink below 2 px.
        let mut sizes = Vec::with_capacity(MAX_LEVELS as usize);
        let mut s = cx.size;
        for _ in 0..levels(values, cx.frame.scale) {
            s = [s[0].div_ceil(2), s[1].div_ceil(2)];
            if s[0] < 2 || s[1] < 2 {
                break;
            }
            sizes.push(s);
        }
        if sizes.is_empty() {
            sizes.push([cx.size[0].div_ceil(2).max(1), cx.size[1].div_ceil(2).max(1)]);
        }
        let mips: Vec<_> = sizes.iter().map(|s| gpu.texture(s[0], s[1], MIP_FORMAT, usage)).collect();

        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        fn view(v: &wgpu::TextureView) -> wgpu::BindingResource<'_> {
            wgpu::BindingResource::TextureView(v)
        }

        let u = gpu.uniform(&PassUniform { texel: texel(cx.size), ..base });
        let bg = gpu.bind(&self.prefilter, &[u.as_entire_binding(), sampler.clone(), view(cx.src)]);
        gpu.draw(cx.encoder, &self.prefilter, &bg, &mips[0].view);

        for i in 1..mips.len() {
            let u = gpu.uniform(&PassUniform { texel: texel(sizes[i - 1]), ..base });
            let bg = gpu.bind(&self.down, &[u.as_entire_binding(), sampler.clone(), view(&mips[i - 1].view)]);
            gpu.draw(cx.encoder, &self.down, &bg, &mips[i].view);
        }

        // ups[i] = mips[i] + tent(ups[i + 1]); the coarsest level is its own "up".
        let mut ups = Vec::with_capacity(mips.len());
        for i in (0..mips.len().saturating_sub(1)).rev() {
            let target = gpu.texture(sizes[i][0], sizes[i][1], MIP_FORMAT, usage);
            let (smaller, smaller_size) = match ups.last() {
                Some(up) => (up, sizes[i + 1]),
                None => (&mips[i + 1], sizes[i + 1]),
            };
            let u = gpu.uniform(&PassUniform { texel: texel(smaller_size), ..base });
            let bg =
                gpu.bind(&self.up, &[u.as_entire_binding(), sampler.clone(), view(&smaller.view), view(&mips[i].view)]);
            gpu.draw(cx.encoder, &self.up, &bg, &target.view);
            ups.push(target);
        }
        let bloom = ups.last().unwrap_or(&mips[0]);

        let u = gpu.uniform(&base);
        let bg = gpu.bind(&self.composite, &[u.as_entire_binding(), sampler, view(&bloom.view), view(cx.src)]);
        gpu.draw(cx.encoder, &self.composite, &bg, cx.dst);
    }
}
