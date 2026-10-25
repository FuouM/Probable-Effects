//! Host pixel buffers, described exactly as the host lays them out. No CPU conversion happens:
//! rows are uploaded as-is and `io.wgsl` swizzles, normalises and decodes on the GPU.

/// Channel order in memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ChannelOrder {
    /// After Effects worlds.
    Argb = 0,
    /// Premiere and most Windows hosts.
    Bgra = 1,
    /// OFX, PNG/EXR.
    Rgba = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Depth {
    U8,
    /// After Effects 16 bpc: 0..=32768 maps to 0..=1.
    U16Ae,
    /// Full-range 16-bit: 0..=65535 maps to 0..=1.
    U16,
    F32,
}

impl Depth {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            Depth::U8 => 4,
            Depth::U16Ae | Depth::U16 => 8,
            Depth::F32 => 16,
        }
    }

    pub(crate) fn texture_format(self) -> wgpu::TextureFormat {
        match self {
            Depth::U8 => wgpu::TextureFormat::Rgba8Unorm,
            Depth::U16Ae | Depth::U16 => wgpu::TextureFormat::Rgba16Uint,
            Depth::F32 => wgpu::TextureFormat::Rgba32Float,
        }
    }

    /// Divisor that maps integer texels to 0..1 (unused for unorm and float).
    pub(crate) fn int_scale(self) -> f32 {
        match self {
            Depth::U16Ae => 32768.0,
            Depth::U16 => 65535.0,
            Depth::U8 | Depth::F32 => 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelLayout {
    pub order: ChannelOrder,
    pub depth: Depth,
}

/// Placement and memory layout shared by [`HostImage`] and [`HostImageMut`].
#[derive(Clone, Copy, Debug)]
pub struct ImageDesc {
    pub width: u32,
    pub height: u32,
    /// Bytes between the starts of consecutive rows in memory (always positive).
    pub stride: usize,
    /// Memory row 0 is the bottom image row (hosts with negative row bytes).
    pub bottom_up: bool,
    /// Colour is not premultiplied by alpha (PNG, some OFX hosts). AE and Premiere are premultiplied.
    pub straight_alpha: bool,
    pub layout: PixelLayout,
    /// Top-left corner in the shared host coordinate space (AE: downsampled layer coordinates).
    pub origin: [i32; 2],
}

impl ImageDesc {
    pub(crate) fn row_bytes(&self) -> usize {
        self.width as usize * self.layout.depth.bytes_per_pixel()
    }

    pub(crate) fn validate(&self, len: usize) -> Result<(), crate::Error> {
        let row = self.row_bytes();
        if self.width == 0 || self.height == 0 {
            return Err(crate::Error::Image(format!("empty image {}x{}", self.width, self.height)));
        }
        if self.stride < row || !self.stride.is_multiple_of(4) {
            return Err(crate::Error::Image(format!("stride {} invalid for row of {row} bytes", self.stride)));
        }
        let need = self.stride * (self.height as usize - 1) + row;
        if len < need {
            return Err(crate::Error::Image(format!("buffer has {len} bytes, layout needs {need}")));
        }
        Ok(())
    }
}

pub struct HostImage<'a> {
    pub data: &'a [u8],
    pub desc: ImageDesc,
}

pub struct HostImageMut<'a> {
    pub data: &'a mut [u8],
    pub desc: ImageDesc,
}
