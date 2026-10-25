# Scaffold a new single-pass effect end to end: effect module + WGSL, registry entry, After
# Effects plugin crate, smoke case. The generated effect is a working pass-through with one
# "Amount" param: edit crates\effects\src\<id>.rs and shaders\<id>.wgsl, then run
# scripts\check.ps1 -Ae.
#   .\scripts\new-effect.ps1 -Id vignette -Type Vignette -Title "Vignette" [-TimeVarying]
#     Id           snake/lowercase; module, shader, CLI name and plugin dir (-> ProbableVignette.aex)
#     Type         Rust type, also the AE match name suffix: "PROBABLE Vignette" (never change later)
#     Title        display name suffix: "Probable Vignette"
#     TimeVarying  output changes with time at fixed params (grain): AE must not cache frames
param(
    [Parameter(Mandatory)][ValidatePattern('^[a-z][a-z0-9]*$')][string]$Id,
    [Parameter(Mandatory)][ValidatePattern('^[A-Z][A-Za-z0-9]*$')][string]$Type,
    [Parameter(Mandatory)][string]$Title,
    [switch]$TimeVarying
)
$ErrorActionPreference = "Stop"
$Root = Split-Path $PSScriptRoot -Parent
$utf8 = New-Object Text.UTF8Encoding($false)
function Write-NewFile($path, $text) {
    if (Test-Path $path) { throw "$path already exists" }
    New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
    [IO.File]::WriteAllText($path, ($text -replace "`r?`n", "`n"), $utf8)
    Write-Host "created $path"
}

$libPath = "$Root\crates\effects\src\lib.rs"
$lib = [IO.File]::ReadAllText($libPath) -replace "`r`n", "`n"
if ($lib -match "mod $Id;") { throw "$Id is already registered in crates\effects\src\lib.rs" }
$tv = if ($TimeVarying) { "true" } else { "false" }

Write-NewFile "$Root\crates\effects\src\$Id.rs" @"
//! Probable ${Title}: TODO one line on what it ports from probable-video-editor.

use probable_core::{Effect, EffectInfo, Gpu, ParamDesc, RenderCx, Values, wgpu};

pub struct $Type {
    pipeline: wgpu::RenderPipeline,
}

const AMOUNT: usize = 0;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniform {
    amount: f32,
    time: f32,
    size: [f32; 2],
}

impl Effect for $Type {
    const INFO: EffectInfo = EffectInfo {
        name: "Probable $Title",
        match_name: "PROBABLE $Type",
        category: "Probable",
        description: "TODO",
    };

    const PARAMS: &'static [ParamDesc] = &[ParamDesc::float(1, "amount", "Amount", 1.0, 0.0, 1.0, 1.0)];

    fn new(gpu: &Gpu) -> Self {
        let module = crate::shader(gpu, "$Id", include_str!("shaders/$Id.wgsl"));
        Self { pipeline: gpu.fullscreen_pipeline(&module, "fs_main", wgpu::TextureFormat::Rgba32Float) }
    }

    fn render(&self, cx: &mut RenderCx<'_>, values: &Values) {
        let gpu = cx.gpu;
        let u = gpu.uniform(&Uniform {
            amount: values.float(AMOUNT),
            time: cx.frame.time as f32,
            size: [cx.size[0] as f32, cx.size[1] as f32],
        });
        let bg = gpu.bind(&self.pipeline, &[u.as_entire_binding(), wgpu::BindingResource::TextureView(cx.src)]);
        gpu.draw(cx.encoder, &self.pipeline, &bg, cx.dst);
    }
}
"@

Write-NewFile "$Root\crates\effects\src\shaders\$Id.wgsl" @"
// $Title. Input and output are linear Rec.709, premultiplied.

struct U {
    amount: f32,
    time: f32,
    size: vec2<f32>,    // canvas size in px
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var src: texture_2d<f32>;

// Add `@group(0) @binding(2) var samp: sampler;` and pass `wgpu::BindingResource::Sampler(..)`
// in render() when the shader filters. Unused bindings are dropped from the layout: every
// declared binding must be referenced, and every one referenced must be bound.
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureLoad(src, vec2<i32>(in.pos.xy), 0) * u.amount;
}
"@

# Registry: module + re-export after the last existing ones, macro entry before its closing `};`.
$lib = [regex]::Replace($lib, '(?s)(.*\nmod \w+;\n)', "`$1mod $Id;`n", 1)
$lib = [regex]::Replace($lib, '(?s)(.*\npub use \w+::\w+;\n)', "`$1pub use ${Id}::${Type};`n", 1)
$lib = [regex]::Replace($lib, '(\n    \};\n\}\s*)$', "`n        `$m!(`$crate::$Type, `"$Id`");`$1")
[IO.File]::WriteAllText($libPath, $lib, $utf8)
Write-Host "registered $Type in crates\effects\src\lib.rs"

$plugin = "$Root\plugins\ae\$Id"
Write-NewFile "$plugin\Cargo.toml" @"
[package]
name = "probable-$Id-ae"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
probable-ae.workspace = true
probable-effects.workspace = true
# define_effect! references ::premiere when built with_premiere.
premiere.workspace = true

[build-dependencies]
probable-ae-build.workspace = true
"@
Write-NewFile "$plugin\build.rs" @"
fn main() {
    probable_ae_build::effect(probable_ae_build::Plugin {
        name: "Probable $Title",
        match_name: "PROBABLE $Type",
        category: "Probable",
        time_varying: $tv,
    });
}
"@
Write-NewFile "$plugin\src\lib.rs" "probable_ae::export_effect!(probable_effects::$Type);`n"

$casesPath = "$Root\tests\ae-smoke-cases.ps1"
$cases = [IO.File]::ReadAllText($casesPath) -replace "`r`n", "`n"
$case = "    @{ Name = `"$Id`"; MatchName = `"PROBABLE $Type`"; AeParams = @{ Amount = 1 }; CliParams = @(`"amount=1`") }"
$cases = [regex]::Replace($cases, '\n\)\s*$', "`n$case`n)`n")
[IO.File]::WriteAllText($casesPath, $cases, $utf8)
Write-Host "added smoke case to tests\ae-smoke-cases.ps1"
Write-Host "next: edit the effect and shader, then .\scripts\check.ps1 -Ae"
