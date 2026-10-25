//! `probable`: run effects on still images, no host application needed.
//!
//! ```text
//! probable list
//! probable render <effect> <in.png|exr> <out.png|exr> [key=value ...] [--time S] [--scale F]
//! probable diff <a> <b> [--tolerance T]
//! probable serve [image] [--port N] [--watch] [--open]
//! ```
//! Host params are set the same way: `space=linear`, `grow=on`.

mod serve;
use std::process::ExitCode;

use probable_cli::{parse_options, render_image};
use probable_core::{Effect, ParamDesc, ParamKind, Processor, host_params};

const USAGE: &str = concat!(
    "usage:
  probable list
  probable render <effect> <in.png|exr> <out.png|exr> [key=value ...] [--time SECONDS] [--scale FACTOR]
  probable diff <a> <b> [--tolerance T]   (T in 0..1 units, default 3/255; exit 1 when exceeded)
",
    "  probable serve [image] [--port N] [--watch] [--open]   (browser preview; --watch reloads shaders)"
);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("list") => {
            list();
            Ok(())
        }
        Some("render") if args.len() >= 4 => render(&args[1], &args[2], &args[3], &args[4..]),
        Some("diff") if args.len() >= 3 => diff(&args[1], &args[2], &args[3..]),
        Some("serve") => serve::serve(&args[1..]),
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
        ParamKind::Color { default } => format!("colour #rrggbb or r,g,b in 0..1, default {default:?}"),
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

fn render_with<E: Effect>(input: &str, output: &str, rest: &[String]) -> Result<(), String> {
    let opts = parse_options::<E>(rest)?;
    let img = probable_cli::load(input)?;
    let started = std::time::Instant::now();
    let out = render_image(&Processor::<E>::default(), &img, &opts, false)?;
    eprintln!(
        "{} {}x{} -> {}x{} in {:.1} ms",
        E::INFO.name,
        img.width,
        img.height,
        out.width,
        out.height,
        started.elapsed().as_secs_f64() * 1e3
    );
    probable_cli::save(output, out.bytes, out.depth, out.width, out.height)
}
