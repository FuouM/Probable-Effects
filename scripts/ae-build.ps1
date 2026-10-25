# Build every After Effects plugin (plugins/ae/*) and stage the .aex files.
#   .\scripts\ae-build.ps1            debug build  -> target\debug\aex\
#   .\scripts\ae-build.ps1 -Release   release build -> target\release\aex\
# Debug builds log to DbgView and show panics in AE; release builds are what users install.
param([switch]$Release)
$ErrorActionPreference = "Stop"
$Root = Split-Path $PSScriptRoot -Parent
. "$Root\env.ps1" | Out-Null

$profileDir = if ($Release) { "release" } else { "debug" }
$cargoArgs = @("build")
if ($Release) { $cargoArgs += "--release" }

$plugins = Get-ChildItem "$Root\plugins\ae" -Directory
foreach ($p in $plugins) {
    $manifest = Get-Content "$($p.FullName)\Cargo.toml" -Raw
    if ($manifest -notmatch 'name\s*=\s*"([^"]+)"') { throw "no package name in $($p.FullName)" }
    $cargoArgs += @("-p", $Matches[1])
}
& "$Root\.rust\.cargo\bin\cargo.exe" @cargoArgs
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$out = "$Root\target\$profileDir\aex"
New-Item -ItemType Directory -Force -Path $out | Out-Null
foreach ($p in $plugins) {
    $manifest = Get-Content "$($p.FullName)\Cargo.toml" -Raw
    $null = $manifest -match 'name\s*=\s*"([^"]+)"'
    $crate = $Matches[1] -replace '-', '_'
    # probable-bloom-ae -> ProbableBloom.aex
    $name = "Probable" + (Get-Culture).TextInfo.ToTitleCase($p.Name) + ".aex"
    Copy-Item -Force "$Root\target\$profileDir\$crate.dll" "$out\$name"
    Write-Host "staged $out\$name"
}
