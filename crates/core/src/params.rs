//! Host-independent parameter schema. Every host adapter (AE, OFX, CLI) builds its UI from these
//! descriptors and hands the effect a [`Values`] list in the same order.

/// One effect parameter.
///
/// `id` is persisted by hosts (AE stores it in projects): never reuse or renumber an id once
/// released. `key` is the CLI / preset name.
#[derive(Clone, Copy, Debug)]
pub struct ParamDesc {
    pub id: u16,
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
}

impl ParamDesc {
    /// Float slider with the usual layout: slider spans `min..=slider_max`, two decimals, valid
    /// range `min..=max`. Spell out [`ParamKind::Float`] for anything else.
    pub const fn float(
        id: u16,
        key: &'static str,
        label: &'static str,
        default: f32,
        min: f32,
        max: f32,
        slider_max: f32,
    ) -> Self {
        Self { id, key, label, kind: ParamKind::Float { default, min, max, slider_min: min, slider_max, precision: 2 } }
    }

    pub const fn choice(
        id: u16,
        key: &'static str,
        label: &'static str,
        options: &'static [&'static str],
        default: u32,
    ) -> Self {
        Self { id, key, label, kind: ParamKind::Choice { options, default } }
    }

    /// Colour as the host's picker shows it (display-referred sRGB, 0..1 per channel).
    pub const fn color(id: u16, key: &'static str, label: &'static str, default: [f32; 3]) -> Self {
        Self { id, key, label, kind: ParamKind::Color { default } }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ParamKind {
    /// `min..=max` is the valid range, `slider_min..=slider_max` the default UI range.
    Float {
        default: f32,
        min: f32,
        max: f32,
        slider_min: f32,
        slider_max: f32,
        precision: u8,
    },
    Bool {
        default: bool,
    },
    /// Index into `options`.
    Choice {
        options: &'static [&'static str],
        default: u32,
    },
    /// Display-referred (sRGB-encoded) 0..1, as a host colour picker shows it. Effects read it
    /// through [`Values::color_linear`].
    Color {
        default: [f32; 3],
    },
}

impl ParamKind {
    pub fn default_value(&self) -> Value {
        match *self {
            ParamKind::Float { default, .. } => Value::Float(default),
            ParamKind::Bool { default } => Value::Bool(default),
            ParamKind::Choice { default, .. } => Value::Choice(default),
            ParamKind::Color { default } => Value::Color(default),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    Bool(bool),
    Choice(u32),
    Color([f32; 3]),
}

/// Parameter values, parallel to an effect's `PARAMS` slice.
#[derive(Clone, Debug, PartialEq)]
pub struct Values(Vec<Value>);

impl Values {
    pub fn defaults(descs: &[ParamDesc]) -> Self {
        Self(descs.iter().map(|d| d.kind.default_value()).collect())
    }

    pub fn set(&mut self, index: usize, value: Value) {
        self.0[index] = value;
    }

    pub fn get(&self, index: usize) -> Value {
        self.0[index]
    }

    /// Panics when the param at `index` is not a float: that is an effect bug, not user input.
    pub fn float(&self, index: usize) -> f32 {
        match self.0[index] {
            Value::Float(v) => v,
            other => panic!("param {index} is {other:?}, expected Float"),
        }
    }

    pub fn bool(&self, index: usize) -> bool {
        match self.0[index] {
            Value::Bool(v) => v,
            other => panic!("param {index} is {other:?}, expected Bool"),
        }
    }

    pub fn choice(&self, index: usize) -> u32 {
        match self.0[index] {
            Value::Choice(v) => v,
            other => panic!("param {index} is {other:?}, expected Choice"),
        }
    }

    /// The colour as picked (sRGB-encoded).
    pub fn color(&self, index: usize) -> [f32; 3] {
        match self.0[index] {
            Value::Color(v) => v,
            other => panic!("param {index} is {other:?}, expected Color"),
        }
    }

    /// The colour in linear light, Rec.709 primaries: what shaders blend with.
    pub fn color_linear(&self, index: usize) -> [f32; 3] {
        self.color(index).map(srgb_to_linear)
    }
}

fn srgb_to_linear(v: f32) -> f32 {
    if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
}

/// `#rrggbb` / `rrggbb` (8-bit) or `r,g,b` (0..1 floats).
fn parse_color(key: &str, text: &str) -> Result<[f32; 3], String> {
    let bad = || format!("{key}: '{text}' is not a colour (#rrggbb or r,g,b in 0..1)");
    let hex = text.strip_prefix('#').unwrap_or(text);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let n = u32::from_str_radix(hex, 16).map_err(|_| bad())?;
        return Ok([(n >> 16) & 255, (n >> 8) & 255, n & 255].map(|c| c as f32 / 255.0));
    }
    let parts: Vec<f32> = text.split(',').map(|p| p.trim().parse().map_err(|_| bad())).collect::<Result<_, _>>()?;
    match parts[..] {
        [r, g, b] if parts.iter().all(|c| (0.0..=1.0).contains(c)) => Ok([r, g, b]),
        _ => Err(bad()),
    }
}

/// Parse a CLI/preset string into a value of the descriptor's kind.
pub fn parse_value(desc: &ParamDesc, text: &str) -> Result<Value, String> {
    match desc.kind {
        ParamKind::Float { min, max, .. } => {
            let v: f32 = text.parse().map_err(|_| format!("{}: '{text}' is not a number", desc.key))?;
            if !(min..=max).contains(&v) {
                return Err(format!("{}: {v} is outside {min}..={max}", desc.key));
            }
            Ok(Value::Float(v))
        }
        ParamKind::Bool { .. } => match text {
            "1" | "true" | "on" => Ok(Value::Bool(true)),
            "0" | "false" | "off" => Ok(Value::Bool(false)),
            _ => Err(format!("{}: '{text}' is not a boolean", desc.key)),
        },
        ParamKind::Choice { options, .. } => {
            // Index, or a unique prefix of the label ignoring case and punctuation:
            // "linear" → "Linear (sRGB primaries)", "bloom-only" → "Bloom Only".
            let fold = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
            let want = fold(text);
            let matches: Vec<usize> = (0..options.len()).filter(|&i| fold(options[i]).starts_with(&want)).collect();
            let index = match (text.parse::<usize>(), matches.as_slice()) {
                (Ok(i), _) if i < options.len() => Some(i),
                (_, [i]) if !want.is_empty() => Some(*i),
                _ => options.iter().position(|o| fold(o) == want),
            };
            index
                .map(|i| Value::Choice(i as u32))
                .ok_or_else(|| format!("{}: '{text}' is not one of {options:?}", desc.key))
        }
        ParamKind::Color { .. } => parse_color(desc.key, text).map(Value::Color),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TINT: ParamDesc = ParamDesc::color(1, "tint", "Tint", [1.0, 0.5, 0.0]);

    #[test]
    fn colour_parses_hex_and_floats() {
        assert_eq!(parse_value(&TINT, "#ff8000"), Ok(Value::Color([1.0, 128.0 / 255.0, 0.0])));
        assert_eq!(parse_value(&TINT, "0.25, 0.5,1"), Ok(Value::Color([0.25, 0.5, 1.0])));
        assert!(parse_value(&TINT, "1,2,0").is_err());
        assert!(parse_value(&TINT, "#ff80").is_err());
    }

    #[test]
    fn picked_colour_is_decoded_to_linear() {
        let mut v = Values::defaults(&[TINT]);
        v.set(0, Value::Color([1.0, 0.5, 0.0]));
        let [r, g, b] = v.color_linear(0);
        assert_eq!((r, b), (1.0, 0.0));
        assert!((g - 0.2140).abs() < 1e-3, "sRGB 0.5 is linear 0.214, got {g}");
    }
}
