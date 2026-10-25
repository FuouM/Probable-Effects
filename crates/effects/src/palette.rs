//! Probable Palette: pve's nine-colour palette (`palette.ts`, `DEFAULT_HEX`) as a set of colour
//! controls. The image passes through unchanged; other layers and effects read the colours with
//! expressions, e.g.
//! `thisComp.layer("Palette").effect("Probable Palette")("Signal")`, so a project recolours in one
//! place, as pve's `palette.json` does.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct Palette {
    pipeline: wgpu::RenderPipeline,
}

const COLOURS: usize = 9;
const SWATCHES: usize = COLOURS;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    /// Linear colours, alpha unused.
    colours: [[f32; 4]; COLOURS],
    swatches: u32,
    _pad: [u32; 3],
}

impl Effect for Palette {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable Palette",
        match_name: "PROBABLE Palette",
        category: "Probable",
        description: "Nine named colours for the whole project; the image passes through. Read them with expressions.",
    };

    // pve's default palette, as picked (sRGB hex values): ink #0A0A0B, ink2 #151517,
    // graphite #5E5B57, ash #9C978F, bone #EEE9DF, signal #FF4D12, ember #FF8A3D,
    // blood #C21D0B, acid #D8FF3C.
    const PARAMS: &'static [ParamDesc] = &[
        ParamDesc::color(1, "ink", "Ink", rgb(0x0A0A0B)),
        ParamDesc::color(2, "ink2", "Ink 2", rgb(0x151517)),
        ParamDesc::color(3, "graphite", "Graphite", rgb(0x5E5B57)),
        ParamDesc::color(4, "ash", "Ash", rgb(0x9C978F)),
        ParamDesc::color(5, "bone", "Bone", rgb(0xEEE9DF)),
        ParamDesc::color(6, "signal", "Signal", rgb(0xFF4D12)),
        ParamDesc::color(7, "ember", "Ember", rgb(0xFF8A3D)),
        ParamDesc::color(8, "blood", "Blood", rgb(0xC21D0B)),
        ParamDesc::color(9, "acid", "Acid", rgb(0xD8FF3C)),
        ParamDesc {
            id: 10,
            key: "swatches",
            label: "Show Swatches",
            kind: probable_core::ParamKind::Bool { default: false },
        },
    ];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "palette", include_str!("shaders/palette.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let mut colours = [[0.0; 4]; COLOURS];
        for (i, c) in colours.iter_mut().enumerate() {
            let [r, g, b] = values.color_linear(i);
            *c = [r, g, b, 1.0];
        }
        let u = gpu.uniform(&Uniform { colours, swatches: u32::from(values.bool(SWATCHES)), _pad: [0; 3] });
        let bg = gpu.bind(&self.pipeline, &[u.as_entire_binding(), wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}

/// `0xRRGGBB` as the 0..1 colour a picker shows.
const fn rgb(hex: u32) -> [f32; 3] {
    [((hex >> 16) & 255) as f32 / 255.0, ((hex >> 8) & 255) as f32 / 255.0, (hex & 255) as f32 / 255.0]
}
