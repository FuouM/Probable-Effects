//! Colour spaces the host pixels can be in. Effects always see linear light with Rec.709/sRGB
//! primaries, premultiplied alpha; `io.wgsl` decodes on the way in and re-encodes on the way out.

/// Order matters: the index is the persisted host param value and the `space` id in `io.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ColorSpace {
    Srgb = 0,
    Linear = 1,
    Gamma24 = 2,
    Rec2020 = 3,
    AcesCg = 4,
    AcesCct = 5,
}

impl ColorSpace {
    pub const ALL: [ColorSpace; 6] = [
        ColorSpace::Srgb,
        ColorSpace::Linear,
        ColorSpace::Gamma24,
        ColorSpace::Rec2020,
        ColorSpace::AcesCg,
        ColorSpace::AcesCct,
    ];

    pub const LABELS: [&'static str; 6] = [
        "sRGB",
        "Linear (sRGB primaries)",
        "Rec.709 / BT.1886 (gamma 2.4)",
        "Rec.2020 (gamma 2.4)",
        "ACEScg (linear AP1)",
        "ACEScct (AP1)",
    ];

    pub fn from_index(i: u32) -> Option<Self> {
        Self::ALL.get(i as usize).copied()
    }

    /// Transfer function id used by `io.wgsl`.
    pub(crate) fn transfer(self) -> u32 {
        match self {
            ColorSpace::Srgb => 0,
            ColorSpace::Linear | ColorSpace::AcesCg => 1,
            ColorSpace::Gamma24 | ColorSpace::Rec2020 => 2,
            ColorSpace::AcesCct => 3,
        }
    }

    /// Row-major matrix taking this space's linear RGB to linear Rec.709 primaries.
    pub fn to_rec709(self) -> [[f32; 3]; 3] {
        match self {
            ColorSpace::Srgb | ColorSpace::Linear | ColorSpace::Gamma24 => IDENTITY,
            ColorSpace::Rec2020 => REC2020_TO_REC709,
            ColorSpace::AcesCg | ColorSpace::AcesCct => AP1_TO_REC709,
        }
    }

    /// Exact inverse of [`Self::to_rec709`], so untouched pixels round-trip.
    pub fn from_rec709(self) -> [[f32; 3]; 3] {
        invert3(self.to_rec709())
    }
}

const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// ITU-R BT.2087 (Rec.2020 → Rec.709, both D65).
const REC2020_TO_REC709: [[f32; 3]; 3] = [
    [1.660_491, -0.587_641_1, -0.072_849_9],
    [-0.124_550_5, 1.132_899_9, -0.008_349_4],
    [-0.018_150_8, -0.100_578_9, 1.118_729_7],
];

/// ACES AP1 (D60) → Rec.709 (D65), Bradford adaptation (ACES reference).
const AP1_TO_REC709: [[f32; 3]; 3] = [
    [1.705_051, -0.621_792_1, -0.083_258_7],
    [-0.130_256_4, 1.140_804_7, -0.010_548_3],
    [-0.024_003_4, -0.128_968_9, 1.152_972_3],
];

fn invert3(m: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let m = m.map(|r| r.map(f64::from));
    let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
    let c01 = m[1][2] * m[2][0] - m[1][0] * m[2][2];
    let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];
    let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;
    let inv = [
        [c00, m[0][2] * m[2][1] - m[0][1] * m[2][2], m[0][1] * m[1][2] - m[0][2] * m[1][1]],
        [c01, m[0][0] * m[2][2] - m[0][2] * m[2][0], m[0][2] * m[1][0] - m[0][0] * m[1][2]],
        [c02, m[0][1] * m[2][0] - m[0][0] * m[2][1], m[0][0] * m[1][1] - m[0][1] * m[1][0]],
    ];
    inv.map(|r| r.map(|v| (v / det) as f32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_stays_white_across_gamuts() {
        // Both matrices map equal-energy white of their space to Rec.709 white (row sums = 1).
        for space in [ColorSpace::Rec2020, ColorSpace::AcesCg] {
            for row in space.to_rec709() {
                assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-4, "{space:?}");
            }
        }
    }
}
