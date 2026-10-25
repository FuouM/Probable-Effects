//! Probable Effects core: host-independent GPU runtime shared by every effect and host adapter.

pub mod color;
pub mod effect;
pub mod gpu;
pub mod image;
mod io;
pub mod params;
mod process;

pub use color::ColorSpace;
pub use effect::{Effect, EffectInfo, Frame, GROW_PARAM, HOST_PARAM_ID_BASE, RenderCx, SPACE_PARAM, host_params};
pub use gpu::{Gpu, PooledTexture, gpu};
pub use image::{ChannelOrder, Depth, HostImage, HostImageMut, ImageDesc, PixelLayout};
pub use params::{ParamDesc, ParamKind, Value, Values, parse_value};
pub use process::Processor;
pub use wgpu;

#[derive(Debug, Clone)]
pub enum Error {
    /// Device creation, validation or readback failure.
    Gpu(String),
    /// Host buffer does not match its description.
    Image(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Gpu(e) => write!(f, "GPU: {e}"),
            Error::Image(e) => write!(f, "image: {e}"),
        }
    }
}

impl std::error::Error for Error {}
