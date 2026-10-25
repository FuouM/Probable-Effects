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
}

impl ParamKind {
    pub fn default_value(&self) -> Value {
        match *self {
            ParamKind::Float { default, .. } => Value::Float(default),
            ParamKind::Bool { default } => Value::Bool(default),
            ParamKind::Choice { default, .. } => Value::Choice(default),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Float(f32),
    Bool(bool),
    Choice(u32),
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
    }
}
