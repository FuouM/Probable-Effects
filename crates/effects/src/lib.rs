//! Every Probable effect. Each type implements [`probable_core::Effect`]; host adapters and the
//! CLI are generic over it.

use probable_core::{Gpu, wgpu};

mod aberration;
mod ascii;
mod bloom;
mod droste;
mod grain;
mod halation;
mod hatch;
mod print;
mod pyramid;
mod shards;
mod shoulder;
mod vignette;

pub use aberration::ChromaticAberration;
pub use ascii::Ascii;
pub use bloom::Bloom;
pub use droste::Droste;
pub use grain::FilmGrain;
pub use halation::Halation;
pub use hatch::Hatch;
pub use print::Print;
pub use shards::MirrorShards;
pub use shoulder::Shoulder;
pub use vignette::Vignette;

/// Compiles `source` with `common.wgsl` prepended (and the fullscreen vertex stage, which
/// `fullscreen_module` adds).
///
/// With `PROBABLE_SHADER_DIR` set, `<dir>/<label>.wgsl` (and `common.wgsl`) replace the embedded
/// text when present: edit a shader and rebuild the effect without recompiling Rust. The preview
/// app does this; hosts never set the variable.
pub(crate) fn shader(gpu: &Gpu, label: &str, source: &str) -> wgpu::ShaderModule {
    let source = shader_text(label, source);
    let common = shader_text("common", include_str!("shaders/common.wgsl"));
    gpu.fullscreen_module(label, &format!("{common}\n{source}"))
}

fn shader_text(label: &str, embedded: &str) -> String {
    std::env::var_os("PROBABLE_SHADER_DIR")
        .and_then(|dir| std::fs::read_to_string(std::path::Path::new(&dir).join(format!("{label}.wgsl"))).ok())
        .unwrap_or_else(|| embedded.to_owned())
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
        $m!($crate::Hatch, "hatch");
        $m!($crate::Print, "print");
        $m!($crate::Ascii, "ascii");
        $m!($crate::Droste, "droste");
        $m!($crate::MirrorShards, "shards");
    };
}
