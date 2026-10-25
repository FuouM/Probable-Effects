//! `probable`: run effects on still images, no host application needed.
//!
//! ```text
//! probable list
//! probable render <effect> <in.png|exr> <out.png|exr> [key=value ...] [--time S] [--scale F]
//! probable diff <a> <b> [--tolerance T]
//! ```
//! Host params are set the same way: `space=linear`, `grow=on`.

use std::path::Path;
use std::process::ExitCode;

use image::{DynamicImage, ImageBuffer, Rgba};
use probable_core::{
    ChannelOrder, ColorSpace, Depth, Effect, Frame, HostImage, HostImageMut, ImageDesc, ParamDesc, ParamKind,
    PixelLayout, Processor, Value, Values, host_params, parse_value,
};

const USAGE: &str = "usage:
  probable list
  probable render <effect> <in.png|exr> <out.png|exr> [key=value ...] [--time SECONDS] [--scale FACTOR]
  probable diff <a> <b> [--tolerance T]   (T in 0..1 units, default 3/255; exit 1 when exceeded)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("list") => {
            list();
            Ok(())
        }
        Some("render") if args.len() >= 4 => render(&args[1], &args[2], &args[3], &args[4..]),
        Some("diff") if args.len() >= 3 => diff(&args[1], &args[2], &args[3..]),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Compare two images channel by channel in normalised 0..1 units.
fn diff(a: &str, b: &str, rest: &[String]) -> Result<(), String> {
    let tolerance = match rest {
        [] => 3.0 / 255.0,
        [flag, v] if flag == "--tolerance" => v.parse::<f32>().map_err(|_| "--tolerance needs a number".to_string())?,
        _ => return Err(USAGE.to_string()),
    };
    let load = |p: &str| image::open(p).map(|i| i.to_rgba32f()).map_err(|e| format!("{p}: {e}"));
    let (ia, ib) = (load(a)?, load(b)?);
    if ia.dimensions() != ib.dimensions() {
        return Err(format!("size differs: {:?} vs {:?}", ia.dimensions(), ib.dimensions()));
    }
    let (mut max, mut sum, mut over) = (0.0f32, 0.0f64, 0usize);
    for (pa, pb) in ia.pixels().zip(ib.pixels()) {
        let d = pa.0.iter().zip(pb.0).map(|(x, y)| (x - y).abs()).fold(0.0f32, f32::max);
        max = max.max(d);
        sum += d as f64;
        over += (d > 1.0 / 255.0) as usize;
    }
    let n = (ia.width() * ia.height()) as f64;
    println!(
        "max {:.2}/255, mean {:.3}/255, {:.2}% pixels off by more than 1/255",
        max * 255.0,
        sum / n * 255.0,
        over as f64 / n * 100.0
    );
    if max > tolerance {
        return Err(format!("max difference {:.2}/255 exceeds tolerance {:.2}/255", max * 255.0, tolerance * 255.0));
    }
    Ok(())
}

fn list() {
    macro_rules! print_effect {
        ($ty:ty, $name:literal) => {{
            println!("{} ({}): {}", $name, <$ty as Effect>::INFO.name, <$ty as Effect>::INFO.description);
            for p in <$ty as Effect>::PARAMS.iter().chain(host_params::<$ty>()) {
                println!("  {:<12} {}", p.key, describe(p));
            }
        }};
    }
    probable_effects::for_each_effect!(print_effect);
}

fn describe(p: &ParamDesc) -> String {
    match p.kind {
        ParamKind::Float { default, min, max, .. } => format!("float {min}..={max}, default {default}"),
        ParamKind::Bool { default } => format!("bool, default {}", if default { "on" } else { "off" }),
        ParamKind::Choice { options, default } => {
            format!("one of {options:?}, default {:?}", options[default as usize])
        }
    }
}

fn render(effect: &str, input: &str, output: &str, rest: &[String]) -> Result<(), String> {
    macro_rules! dispatch {
        ($ty:ty, $name:literal) => {
            if effect == $name {
                return render_with::<$ty>(input, output, rest);
            }
        };
    }
    probable_effects::for_each_effect!(dispatch);
    Err(format!("unknown effect '{effect}' (see `probable list`)"))
}

struct Options {
    values: Values,
    frame: Frame,
}

fn parse_options<E: Effect>(rest: &[String]) -> Result<Options, String> {
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

fn render_with<E: Effect>(input: &str, output: &str, rest: &[String]) -> Result<(), String> {
    let opts = parse_options::<E>(rest)?;
    let img = image::open(input).map_err(|e| format!("{input}: {e}"))?;
    let (bytes, depth, width, height) = match img {
        DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
            let b = img.to_rgba32f();
            let (w, h) = b.dimensions();
            (f32_bytes(b.into_raw()), Depth::F32, w, h)
        }
        DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgba16(_) => {
            let b = img.to_rgba16();
            let (w, h) = b.dimensions();
            (b.into_raw().iter().flat_map(|v| v.to_ne_bytes()).collect(), Depth::U16, w, h)
        }
        _ => {
            let b = img.to_rgba8();
            let (w, h) = b.dimensions();
            (b.into_raw(), Depth::U8, w, h)
        }
    };
    let layout = PixelLayout { order: ChannelOrder::Rgba, depth };
    let bpp = depth.bytes_per_pixel();
    // EXR stores linear scene values with straight alpha in practice; PNG is straight too.
    let src_desc = ImageDesc {
        width,
        height,
        stride: width as usize * bpp,
        bottom_up: false,
        straight_alpha: true,
        layout,
        origin: [0, 0],
    };
    let m = if opts.frame.grow { E::margin(&opts.values, opts.frame.scale) } else { [0, 0] };
    let (dw, dh) = (width + 2 * m[0], height + 2 * m[1]);
    let dst_desc = ImageDesc {
        width: dw,
        height: dh,
        stride: dw as usize * bpp,
        origin: [-(m[0] as i32), -(m[1] as i32)],
        ..src_desc
    };
    let mut out = vec![0u8; dst_desc.stride * dh as usize];

    let processor = Processor::<E>::default();
    let started = std::time::Instant::now();
    processor
        .render(
            &HostImage { data: &bytes, desc: src_desc },
            &mut HostImageMut { data: &mut out, desc: dst_desc },
            &opts.frame,
            &opts.values,
        )
        .map_err(|e| e.to_string())?;
    eprintln!("{} {width}x{height} -> {dw}x{dh} in {:.1} ms", E::INFO.name, started.elapsed().as_secs_f64() * 1e3);

    save(output, out, depth, dw, dh)
}

fn f32_bytes(v: Vec<f32>) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

fn save(path: &str, data: Vec<u8>, depth: Depth, w: u32, h: u32) -> Result<(), String> {
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
