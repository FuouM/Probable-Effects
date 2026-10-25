# One-time setup of the repo-local Rust toolchain in .rust/ (no global install, no PATH edits).
$ErrorActionPreference = "Stop"
$RootPath = $PSScriptRoot
$RustDir = Join-Path $RootPath ".rust"
New-Item -ItemType Directory -Force -Path "$RustDir\.rustup", "$RustDir\.cargo" | Out-Null

$env:RUSTUP_HOME = "$RustDir\.rustup"
$env:CARGO_HOME = "$RustDir\.cargo"
$BinPath = "$RustDir\.cargo\bin"

if (-not (Test-Path "$BinPath\cargo.exe")) {
    Write-Host "Downloading rustup-init.exe..." -ForegroundColor Cyan
    curl.exe -L -o "$RustDir\rustup-init.exe" https://win.rustup.rs/x86_64
    Write-Host "Installing Rust locally..." -ForegroundColor Cyan
    & "$RustDir\rustup-init.exe" -y --no-modify-path --profile minimal --default-toolchain stable --component rustfmt,clippy
    if ($LASTEXITCODE -ne 0) { throw "rustup-init failed" }
} else {
    Write-Host "Rust already installed, updating stable..." -ForegroundColor DarkGray
    & "$BinPath\rustup.exe" update stable
}

# sccache: prebuilt release binary (compiling it takes minutes).
$SccachePath = "$BinPath\sccache.exe"
if (-not (Test-Path $SccachePath)) {
    Write-Host "Installing sccache..." -ForegroundColor Cyan
    $release = Invoke-RestMethod "https://api.github.com/repos/mozilla/sccache/releases/latest"
    $asset = $release.assets | Where-Object { $_.name -like "*x86_64-pc-windows-msvc.zip" } | Select-Object -First 1
    $zip = "$RustDir\sccache.zip"
    curl.exe -L -o $zip $asset.browser_download_url
    $extract = "$RustDir\sccache-extract"
    Expand-Archive -Force $zip $extract
    Copy-Item (Get-ChildItem $extract -Recurse -Filter sccache.exe | Select-Object -First 1).FullName $SccachePath
    Remove-Item -Recurse -Force $zip, $extract
}

# env.yaml: machine paths for scripts\*.ps1, detected once here so the scripts do no lookups.
# Never overwritten: edit it by hand afterwards (keys: env.example.yaml).
$EnvYaml = Join-Path $RootPath "env.yaml"
if (-not (Test-Path $EnvYaml)) {
    $afterFx = (Get-ItemProperty "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\AfterFX.exe" -ErrorAction SilentlyContinue)."(default)"
    $aeDir = if ($afterFx) { Split-Path $afterFx -Parent } else { "<After Effects install>/Support Files" }
    $pluginDir = Join-Path $env:ProgramFiles "Adobe\Common\Plug-ins\7.0\MediaCore\Probable"
    @(
        "# Machine-specific paths for scripts\*.ps1 (keys documented in env.example.yaml)."
        "ae_support_dir: $($aeDir -replace '\\', '/')"
        "plugin_dir: $($pluginDir -replace '\\', '/')"
        "smoke_image: tests/fixtures/smoke.png"
    ) | Set-Content -Encoding UTF8 $EnvYaml
    Write-Host "Wrote $EnvYaml" -ForegroundColor Cyan
    if (-not $afterFx) { Write-Host "After Effects not found: set ae_support_dir in env.yaml" -ForegroundColor Yellow }
}

Write-Host "Local Rust toolchain ready in $RustDir. Load it with: . .\env.ps1" -ForegroundColor Green
