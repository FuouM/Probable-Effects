//! Every Probable effect. Each type implements [`probable_core::Effect`]; host adapters and the
//! CLI are generic over it.

use probable_core::{Gpu, wgpu};

mod aberration;
mod bloom;
mod grain;
mod halation;
mod pyramid;
mod shoulder;
mod vignette;

pub use aberration::ChromaticAberration;
pub use bloom::Bloom;
pub use grain::FilmGrain;
pub use halation::Halation;
pub use shoulder::Shoulder;
pub use vignette::Vignette;

/// Compiles `source` with `common.wgsl` prepended (and the fullscreen vertex stage, which
/// `fullscreen_module` adds).
pub(crate) fn shader(gpu: &Gpu, label: &str, source: &str) -> wgpu::ShaderModule {
    gpu.fullscreen_module(label, &format!("{}\n{source}", include_str!("shaders/common.wgsl")))
}

/// Calls `$m!(Type, "cli-name")` once per effect. The CLI uses it to build its registry; add
/// new effects here and to `plugins/ae/`.
#[macro_export]
macro_rules! for_each_effect {
    ($m:ident) => {
        $m!($crate::Bloom, "bloom");
        $m!($crate::Shoulder, "shoulder");
        $m!($crate::Vignette, "vignette");
        $m!($crate::ChromaticAberration, "aberration");
        $m!($crate::FilmGrain, "grain");
        $m!($crate::Halation, "halation");
    };
}
