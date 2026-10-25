//! Every Probable effect. Each type implements [`probable_core::Effect`]; host adapters and the
//! CLI are generic over it.

mod bloom;

pub use bloom::Bloom;

/// Calls `$m!(Type, "cli-name")` once per effect. The CLI uses it to build its registry; add
/// new effects here and to `plugins/ae/`.
#[macro_export]
macro_rules! for_each_effect {
    ($m:ident) => {
        $m!($crate::Bloom, "bloom");
    };
}
