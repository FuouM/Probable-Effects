//! The bloom pyramid from pve's post.ts, shared by Bloom and Halation: a soft-threshold
//! prefilter into half resolution, successive 13-tap downsamples, and tent upsamples that add
//! each level back (`ups[i] = mips[i] + tent(ups[i + 1])`).

use probable_core::{Gpu, PooledTexture, RenderCx, wgpu};

/// pve uses half-float targets.
const MIP_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const MAX_LEVELS: u32 = 10;

pub(crate) struct Pyramid {
    prefilter: wgpu::RenderPipeline,
    down: wgpu::RenderPipeline,
    up: wgpu::RenderPipeline,
}

pub(crate) struct Settings {
    pub threshold: f32,
    pub knee: f32,
    /// Tent spread of the upsample, in source texels.
    pub radius: f32,
    /// Coarsest level's texel in full-resolution px (see [`levels`]).
    pub size_px: f32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    texel: [f32; 2],
    threshold: f32,
    knee: f32,
    radius: f32,
    _pad: f32,
}

/// Pyramid depth for `size_px` (full-res px) at host `scale`: the coarsest level's texel is about
/// `size_px`, as pve's 7 levels from 1920x1080 reach 128 px.
pub(crate) fn levels(size_px: f32, scale: [f32; 2]) -> u32 {
    let px = size_px * 0.5 * (scale[0] + scale[1]);
    (px.max(2.0).log2().round() as u32).clamp(1, MAX_LEVELS)
}

/// Pixels the pyramid's light reaches past a bright edge: about two coarsest texels, widened by
/// the tent radius.
pub(crate) fn reach(size_px: f32, scale: [f32; 2], radius: f32) -> u32 {
    (2.0 * (1u32 << levels(size_px, scale)) as f32 * radius).ceil() as u32
}

/// A built pyramid; [`Self::level`] reads the accumulated levels.
pub(crate) struct Built<'g> {
    mips: Vec<PooledTexture<'g>>,
    /// Upsampled levels, coarse to fine: `ups[0]` is level `len - 2`.
    ups: Vec<PooledTexture<'g>>,
}

impl Built<'_> {
    pub fn len(&self) -> usize {
        self.mips.len()
    }

    /// Level `i` (0 = half resolution) including every coarser level's light. The coarsest
    /// level is just its mip.
    pub fn level(&self, i: usize) -> &wgpu::TextureView {
        let n = self.mips.len();
        match n - 1 - i {
            0 => &self.mips[n - 1].view,
            j => &self.ups[j - 1].view,
        }
    }
}

fn view(v: &wgpu::TextureView) -> wgpu::BindingResource<'_> {
    wgpu::BindingResource::TextureView(v)
}

impl Pyramid {
    pub fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "pyramid", include_str!("shaders/pyramid.wgsl"));
        Self {
            prefilter: gpu.fullscreen_pipeline(&module, "fs_prefilter", MIP_FORMAT),
            down: gpu.fullscreen_pipeline(&module, "fs_down", MIP_FORMAT),
            up: gpu.fullscreen_pipeline(&module, "fs_up", MIP_FORMAT),
        }
    }

    pub fn build<'g>(&self, cx: &mut RenderCx<'g>, s: &Settings) -> Built<'g> {
        let gpu = cx.gpu;
        let usage = wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let base = Uniform { texel: [0.0; 2], threshold: s.threshold, knee: s.knee, radius: s.radius, _pad: 0.0 };
        let texel = |size: [u32; 2]| [1.0 / size[0] as f32, 1.0 / size[1] as f32];

        // Stop before a level would shrink below 2 px.
        let mut sizes = Vec::with_capacity(MAX_LEVELS as usize);
        let mut size = cx.size;
        for _ in 0..levels(s.size_px, cx.frame.scale) {
            size = [size[0].div_ceil(2), size[1].div_ceil(2)];
            if size[0] < 2 || size[1] < 2 {
                break;
            }
            sizes.push(size);
        }
        if sizes.is_empty() {
            sizes.push([cx.size[0].div_ceil(2).max(1), cx.size[1].div_ceil(2).max(1)]);
        }
        let mips: Vec<_> = sizes.iter().map(|s| gpu.texture(s[0], s[1], MIP_FORMAT, usage)).collect();

        let sampler = wgpu::BindingResource::Sampler(&gpu.linear_clamp);
        let u = gpu.uniform(&Uniform { texel: texel(cx.size), ..base });
        let bg = gpu.bind(&self.prefilter, &[u.as_entire_binding(), sampler.clone(), view(cx.src)]);
        gpu.draw(cx.encoder, &self.prefilter, &bg, &mips[0].view);

        for i in 1..mips.len() {
            let u = gpu.uniform(&Uniform { texel: texel(sizes[i - 1]), ..base });
            let bg = gpu.bind(&self.down, &[u.as_entire_binding(), sampler.clone(), view(&mips[i - 1].view)]);
            gpu.draw(cx.encoder, &self.down, &bg, &mips[i].view);
        }

        let mut ups: Vec<PooledTexture<'g>> = Vec::with_capacity(mips.len());
        for i in (0..mips.len() - 1).rev() {
            let target = gpu.texture(sizes[i][0], sizes[i][1], MIP_FORMAT, usage);
            let smaller = ups.last().map_or(&mips[i + 1].view, |up| &up.view);
            let u = gpu.uniform(&Uniform { texel: texel(sizes[i + 1]), ..base });
            let bg = gpu.bind(&self.up, &[u.as_entire_binding(), sampler.clone(), view(smaller), view(&mips[i].view)]);
            gpu.draw(cx.encoder, &self.up, &bg, &target.view);
            ups.push(target);
        }
        Built { mips, ups }
    }
}
