//! What `probable render` and `probable serve` share: still-image IO, `key=value` options, and
//! one render of a loaded image through an effect.

use std::path::Path;

use image::{DynamicImage, ImageBuffer, Rgba};
use probable_core::{
    ChannelOrder, ColorSpace, Depth, Effect, Frame, HostImage, HostImageMut, ImageDesc, PixelLayout, Processor, Value,
    Values, host_params, parse_value,
};

/// Pixels as the effects want them: RGBA, straight alpha, at the file's own precision.
pub struct Loaded {
    pub bytes: Vec<u8>,
    pub depth: Depth,
    pub width: u32,
    pub height: u32,
}

/// PNG/JPEG/... load as 8 or 16 bit, EXR as 32-bit float.
pub fn load(path: &str) -> Result<Loaded, String> {
    from_dynamic(image::open(path).map_err(|e| format!("{path}: {e}"))?)
}

/// Same as [`load`] for an encoded image in memory (a browser upload).
pub fn load_bytes(bytes: &[u8]) -> Result<Loaded, String> {
    from_dynamic(image::load_from_memory(bytes).map_err(|e| e.to_string())?)
}

fn from_dynamic(img: DynamicImage) -> Result<Loaded, String> {
    Ok(match img {
        DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
            let b = img.to_rgba32f();
            let (width, height) = b.dimensions();
            Loaded { bytes: f32_bytes(b.into_raw()), depth: Depth::F32, width, height }
        }
        DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgba16(_) => {
            let b = img.to_rgba16();
            let (width, height) = b.dimensions();
            let bytes = b.into_raw().iter().flat_map(|v| v.to_ne_bytes()).collect();
            Loaded { bytes, depth: Depth::U16, width, height }
        }
        _ => {
            let b = img.to_rgba8();
            let (width, height) = b.dimensions();
            Loaded { bytes: b.into_raw(), depth: Depth::U8, width, height }
        }
    })
}

fn f32_bytes(v: Vec<f32>) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

pub fn save(path: &str, data: Vec<u8>, depth: Depth, w: u32, h: u32) -> Result<(), String> {
    let ext = Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let img = match depth {
        Depth::U8 => DynamicImage::ImageRgba8(ImageBuffer::<Rgba<u8>, _>::from_raw(w, h, data).unwrap()),
        Depth::U16 | Depth::U16Ae => {
            let px = data.as_chunks::<2>().0.iter().map(|c| u16::from_ne_bytes(*c)).collect();
            DynamicImage::ImageRgba16(ImageBuffer::<Rgba<u16>, _>::from_raw(w, h, px).unwrap())
        }
        Depth::F32 => {
            let px = data.as_chunks::<4>().0.iter().map(|c| f32::from_ne_bytes(*c)).collect();
            DynamicImage::ImageRgba32F(ImageBuffer::<Rgba<f32>, _>::from_raw(w, h, px).unwrap())
        }
    };
    let ok = matches!((depth, ext.as_str()), (Depth::F32, "exr") | (Depth::U8 | Depth::U16 | Depth::U16Ae, "png"));
    if !ok {
        return Err(format!(
            "{depth:?} output must be written as {}",
            if depth == Depth::F32 { ".exr" } else { ".png" }
        ));
    }
    img.save(path).map_err(|e| format!("{path}: {e}"))
}

pub struct Options {
    pub values: Values,
    pub frame: Frame,
}

/// `key=value` pairs for the effect's and the host's params, plus `--time S` and `--scale F`.
pub fn parse_options<E: Effect>(rest: &[String]) -> Result<Options, String> {
    let host = host_params::<E>();
    let mut values = Values::defaults(E::PARAMS);
    let mut host_values = Values::defaults(host);
    let mut time = 0.0;
    let mut scale = 1.0;
    let mut it = rest.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--time" => time = number(it.next(), "--time")?,
            "--scale" => scale = number(it.next(), "--scale")? as f32,
            kv => {
                let (key, text) = kv.split_once('=').ok_or_else(|| format!("expected key=value, got '{kv}'"))?;
                if let Some(i) = E::PARAMS.iter().position(|p| p.key == key) {
                    values.set(i, parse_value(&E::PARAMS[i], text)?);
                } else if let Some(i) = host.iter().position(|p| p.key == key) {
                    host_values.set(i, parse_value(&host[i], text)?);
                } else {
                    return Err(format!("unknown param '{key}' (see `probable list`)"));
                }
            }
        }
    }
    let Value::Choice(space) = host_values.get(0) else { unreachable!("space is a choice") };
    let grow = host.len() > 1 && host_values.bool(1);
    let frame = Frame { time, scale: [scale; 2], space: ColorSpace::from_index(space).unwrap(), grow };
    Ok(Options { values, frame })
}

fn number(v: Option<&String>, flag: &str) -> Result<f64, String> {
    v.and_then(|s| s.parse().ok()).ok_or_else(|| format!("{flag} needs a number"))
}

pub struct Rendered {
    pub bytes: Vec<u8>,
    pub depth: Depth,
    pub width: u32,
    pub height: u32,
    /// Where the source's top-left pixel lies in the result (the Grow Bounds margin).
    pub origin: [u32; 2],
}

/// Render `img` through `processor`. The result keeps the input's depth and working space, or,
/// with `display`, is 8-bit sRGB whatever the input was.
pub fn render_image<E: Effect>(
    processor: &Processor<E>,
    img: &Loaded,
    opts: &Options,
    display: bool,
) -> Result<Rendered, String> {
    let Loaded { bytes, depth, width, height } = img;
    let (width, height) = (*width, *height);
    // EXR stores linear scene values with straight alpha in practice; PNG is straight too.
    let src_desc = ImageDesc {
        width,
        height,
        stride: width as usize * depth.bytes_per_pixel(),
        bottom_up: false,
        straight_alpha: true,
        layout: PixelLayout { order: ChannelOrder::Rgba, depth: *depth },
        origin: [0, 0],
    };
    let m = if opts.frame.grow { E::margin(&opts.values, opts.frame.scale) } else { [0, 0] };
    let out_depth = if display { Depth::U8 } else { *depth };
    let (dw, dh) = (width + 2 * m[0], height + 2 * m[1]);
    let dst_desc = ImageDesc {
        width: dw,
        height: dh,
        stride: dw as usize * out_depth.bytes_per_pixel(),
        origin: [-(m[0] as i32), -(m[1] as i32)],
        layout: PixelLayout { order: ChannelOrder::Rgba, depth: out_depth },
        ..src_desc
    };
    let mut out = vec![0u8; dst_desc.stride * dh as usize];
    let out_space = if display { ColorSpace::Srgb } else { opts.frame.space };
    processor
        .render_to(
            &HostImage { data: bytes, desc: src_desc },
            &mut HostImageMut { data: &mut out, desc: dst_desc },
            &opts.frame,
            out_space,
            &opts.values,
        )
        .map_err(|e| e.to_string())?;
    Ok(Rendered { bytes: out, depth: out_depth, width: dw, height: dh, origin: m })
}
