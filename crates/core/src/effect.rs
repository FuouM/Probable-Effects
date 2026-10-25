//! The contract every effect implements. Effects only record GPU commands; host I/O, colour
//! conversion and readback live in [`crate::Processor`], so the same effect code runs in
//! AE, OFX, the CLI and (later) the browser catalog.

use crate::color::ColorSpace;
use crate::gpu::Gpu;
use crate::params::{ParamDesc, ParamKind, Values};

pub struct EffectInfo {
    /// Display name, e.g. "Probable Bloom".
    pub name: &'static str,
    /// Stable host identifier (AE match name). Never change after release.
    pub match_name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
}

/// Per-frame context from the host.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    /// Seconds (layer time in AE).
    pub time: f64,
    /// Host downsample factor per axis (1 = full resolution, 0.5 = half-res preview). Pixel-sized
    /// params must be multiplied by it so previews match the full render.
    pub scale: [f32; 2],
    pub space: ColorSpace,
    /// Output may extend past the input bounds by [`Effect::margin`].
    pub grow: bool,
}

pub struct RenderCx<'a> {
    pub gpu: &'a Gpu,
    pub encoder: &'a mut wgpu::CommandEncoder,
    /// Input canvas: linear Rec.709 primaries, premultiplied, rgba32float, filterable.
    pub src: &'a wgpu::TextureView,
    /// Output canvas, same size and format as `src`; render attachment. Must be fully written.
    pub dst: &'a wgpu::TextureView,
    pub size: [u32; 2],
    pub frame: &'a Frame,
}

pub trait Effect: Sized + Send + Sync + 'static {
    const INFO: EffectInfo;
    /// Effect params in UI order. Ids below [`HOST_PARAM_ID_BASE`].
    const PARAMS: &'static [ParamDesc];
    /// Whether the effect paints outside its input (offers the Grow Bounds toggle).
    const CAN_GROW: bool = false;

    /// Build pipelines. Called once per process.
    fn new(gpu: &Gpu) -> Self;

    /// Pixels the output can extend past the input on each side, at host scale.
    fn margin(_values: &Values, _scale: [f32; 2]) -> [u32; 2] {
        [0, 0]
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values);
}

/// Ids at or above this belong to params every host adds after the effect's own.
pub const HOST_PARAM_ID_BASE: u16 = 900;

pub const SPACE_PARAM: ParamDesc = ParamDesc {
    id: HOST_PARAM_ID_BASE,
    key: "space",
    label: "Working Space",
    kind: ParamKind::Choice { options: &ColorSpace::LABELS, default: 0 },
};

pub const GROW_PARAM: ParamDesc = ParamDesc {
    id: HOST_PARAM_ID_BASE + 1,
    key: "grow",
    label: "Grow Bounds",
    kind: ParamKind::Bool { default: false },
};

/// Host-added params for `E`, in UI order after `E::PARAMS`.
pub fn host_params<E: Effect>() -> &'static [ParamDesc] {
    const WITH_GROW: [ParamDesc; 2] = [SPACE_PARAM, GROW_PARAM];
    if E::CAN_GROW { &WITH_GROW } else { &WITH_GROW[..1] }
}
