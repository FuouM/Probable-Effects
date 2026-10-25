//! `probable serve`: a local web page with sliders for every effect, rendered by the same native
//! wgpu code as `probable render`. With `--watch` the shaders (and the page) are re-read from the
//! source tree, so editing a `.wgsl` file updates the open page within a moment.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use probable_cli::{Loaded, Rendered, load, load_bytes, parse_options, render_image};
use probable_core::{Depth, Effect, ParamDesc, ParamKind, Processor, host_params};
use tiny_http::{Header, Method, Request, Response, Server};

const PAGE: &str = include_str!("serve.html");
const SHADER_DIR: &str = "crates/effects/src/shaders";
const PAGE_FILE: &str = "crates/cli/src/serve.html";

trait Slot {
    fn key(&self) -> &'static str;
    fn describe(&self, out: &mut String);
    fn render(&self, img: &Loaded, args: &[String]) -> Result<Rendered, String>;
    /// Drop the compiled effect: the next render recompiles its shaders.
    fn reload(&mut self);
}

struct Typed<E: Effect> {
    key: &'static str,
    processor: Processor<E>,
}

impl<E: Effect> Slot for Typed<E> {
    fn key(&self) -> &'static str {
        self.key
    }

    fn describe(&self, out: &mut String) {
        let _ = write!(
            out,
            "{{\"key\":{},\"name\":{},\"description\":{},\"params\":[",
            json_str(self.key),
            json_str(E::INFO.name),
            json_str(E::INFO.description)
        );
        param_list(out, E::PARAMS);
        out.push_str("],\"host\":[");
        param_list(out, host_params::<E>());
        out.push_str("]}");
    }

    fn render(&self, img: &Loaded, args: &[String]) -> Result<Rendered, String> {
        let opts = parse_options::<E>(args)?;
        render_image(&self.processor, img, &opts, true)
    }

    fn reload(&mut self) {
        self.processor = Processor::default();
    }
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn hex(c: [f32; 3]) -> String {
    let [r, g, b] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    format!("#{r:02x}{g:02x}{b:02x}")
}

fn param_list(out: &mut String, params: &[ParamDesc]) {
    for (i, p) in params.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{{\"key\":{},\"label\":{},", json_str(p.key), json_str(p.label));
        match p.kind {
            ParamKind::Float { default, min, max, slider_min, slider_max, precision } => {
                let _ = write!(
                    out,
                    "\"kind\":\"float\",\"default\":{default},\"min\":{min},\"max\":{max},\
                     \"slider_min\":{slider_min},\"slider_max\":{slider_max},\"precision\":{precision}}}"
                );
            }
            ParamKind::Bool { default } => {
                let _ = write!(out, "\"kind\":\"bool\",\"default\":{default}}}");
            }
            ParamKind::Choice { options, default } => {
                let opts: Vec<String> = options.iter().map(|o| json_str(o)).collect();
                let _ = write!(out, "\"kind\":\"choice\",\"options\":[{}],\"default\":{default}}}", opts.join(","));
            }
            ParamKind::Color { default } => {
                let _ = write!(out, "\"kind\":\"color\",\"default\":{}}}", json_str(&hex(default)));
            }
        }
    }
}

struct State {
    image: Loaded,
    image_name: String,
    slots: Vec<Box<dyn Slot>>,
    watch: Option<PathBuf>,
    shaders_seen: Option<SystemTime>,
    /// Bumped when shaders or the image change; the page polls it.
    version: u64,
}

impl State {
    fn new(image: Loaded, image_name: String, watch: Option<PathBuf>) -> Self {
        let mut slots: Vec<Box<dyn Slot>> = Vec::new();
        macro_rules! register {
            ($ty:ty, $name:literal) => {
                slots.push(Box::new(Typed::<$ty> { key: $name, processor: Processor::default() }));
            };
        }
        probable_effects::for_each_effect!(register);
        let shaders_seen = watch.as_deref().and_then(newest_mtime);
        Self { image, image_name, slots, watch, shaders_seen, version: 1 }
    }

    /// Recompile everything when a shader file changed since the last look.
    fn check_shaders(&mut self) {
        let Some(dir) = &self.watch else { return };
        let now = newest_mtime(dir);
        if now != self.shaders_seen {
            self.shaders_seen = now;
            for slot in &mut self.slots {
                slot.reload();
            }
            self.version += 1;
            eprintln!("shaders changed: reloading");
        }
    }

    fn effects_json(&self) -> String {
        let mut out = String::from("{\"effects\":[");
        for (i, slot) in self.slots.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            slot.describe(&mut out);
        }
        out.push_str("],");
        out.push_str(&self.state_json()[1..]);
        out
    }

    fn state_json(&self) -> String {
        format!(
            "{{\"version\":{},\"image\":{{\"name\":{},\"width\":{},\"height\":{},\"depth\":\"{:?}\"}},\"watch\":{}}}",
            self.version,
            json_str(&self.image_name),
            self.image.width,
            self.image.height,
            self.image.depth,
            self.watch.is_some()
        )
    }
}

fn newest_mtime(dir: &Path) -> Option<SystemTime> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "wgsl"))
        .filter_map(|e| e.metadata().ok()?.modified().ok())
        .max()
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() && b[i + 1].is_ascii_hexdigit() && b[i + 2].is_ascii_hexdigit() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 2;
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn split_url(url: &str) -> (&str, Vec<(String, String)>) {
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    let pairs = query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (pct_decode(k), pct_decode(v))
        })
        .collect();
    (path, pairs)
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header")
}

fn text(status: u16, body: impl Into<String>) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body.into()).with_status_code(status).with_header(header("Content-Type", "text/plain"))
}

fn json(body: String) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(body).with_header(header("Content-Type", "application/json"))
}

fn handle(state: &mut State, req: &mut Request) -> Response<std::io::Cursor<Vec<u8>>> {
    let url = req.url().to_string();
    let (path, query) = split_url(&url);
    state.check_shaders();
    match (req.method(), path) {
        (Method::Get, "/") => {
            // --watch serves the page from the tree so editing it needs only a refresh.
            let page = state
                .watch
                .as_ref()
                .and_then(|_| std::fs::read_to_string(PAGE_FILE).ok())
                .unwrap_or_else(|| PAGE.to_string());
            Response::from_string(page).with_header(header("Content-Type", "text/html; charset=utf-8"))
        }
        (Method::Get, "/api/effects") => json(state.effects_json()),
        (Method::Get, "/api/state") => json(state.state_json()),
        (Method::Post, "/api/image") => {
            let mut body = Vec::new();
            if let Err(e) = req.as_reader().read_to_end(&mut body) {
                return text(400, e.to_string());
            }
            match load_bytes(&body) {
                Ok(img) => {
                    state.image = img;
                    state.image_name =
                        query.iter().find(|(k, _)| k == "name").map(|(_, v)| v.clone()).unwrap_or_default();
                    state.version += 1;
                    json(state.state_json())
                }
                Err(e) => text(400, e),
            }
        }
        (Method::Get | Method::Post, "/api/render") => {
            let mut effect = "";
            let mut args = Vec::new();
            for (k, v) in &query {
                match k.as_str() {
                    "effect" => effect = v,
                    "time" => {
                        args.push("--time".to_string());
                        args.push(v.clone());
                    }
                    _ => args.push(format!("{k}={v}")),
                }
            }
            let Some(slot) = state.slots.iter().find(|s| s.key() == effect) else {
                return text(404, format!("unknown effect '{effect}'"));
            };
            let started = Instant::now();
            match slot.render(&state.image, &args) {
                Ok(r) => {
                    debug_assert!(r.depth == Depth::U8);
                    Response::from_data(r.bytes)
                        .with_header(header("Content-Type", "application/octet-stream"))
                        .with_header(header("X-Width", &r.width.to_string()))
                        .with_header(header("X-Height", &r.height.to_string()))
                        .with_header(header("X-Origin-X", &r.origin[0].to_string()))
                        .with_header(header("X-Origin-Y", &r.origin[1].to_string()))
                        .with_header(header("X-Ms", &format!("{:.1}", started.elapsed().as_secs_f64() * 1e3)))
                        .with_header(header(
                            "Access-Control-Expose-Headers",
                            "X-Width, X-Height, X-Origin-X, X-Origin-Y, X-Ms",
                        ))
                }
                Err(e) => text(400, e),
            }
        }
        _ => text(404, "not found"),
    }
}

pub fn serve(args: &[String]) -> Result<(), String> {
    let mut image_path = None;
    let mut port = 7878u16;
    let mut watch = false;
    let mut open = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--port" => port = it.next().and_then(|p| p.parse().ok()).ok_or("--port needs a number")?,
            "--watch" => watch = true,
            "--open" => open = true,
            p if !p.starts_with("--") => image_path = Some(p.to_string()),
            other => return Err(format!("unknown option '{other}'")),
        }
    }

    let image_path = image_path.unwrap_or_else(|| "tests/fixtures/smoke.png".to_string());
    let image = load(&image_path)?;
    let name = Path::new(&image_path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

    let watch_dir = watch.then(|| PathBuf::from(SHADER_DIR));
    if let Some(dir) = &watch_dir {
        if !dir.is_dir() {
            return Err(format!("--watch needs to run from the repo root ({} not found)", dir.display()));
        }
        // SAFETY: still single-threaded here: no server, GPU or worker thread exists yet.
        unsafe { std::env::set_var("PROBABLE_SHADER_DIR", dir) };
    }

    let mut state = State::new(image, name, watch_dir);
    let server = Server::http(("127.0.0.1", port)).map_err(|e| format!("cannot listen on port {port}: {e}"))?;
    let url = format!("http://127.0.0.1:{port}/");
    eprintln!("probable serve: {url}{}", if watch { "  (watching shaders)" } else { "" });
    if open {
        let _ = std::process::Command::new("cmd").args(["/C", "start", "", &url]).spawn();
    }
    for mut req in server.incoming_requests() {
        let response = handle(&mut state, &mut req);
        if let Err(e) = req.respond(response) {
            eprintln!("response failed: {e}");
        }
    }
    Ok(())
}
