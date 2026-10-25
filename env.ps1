# Load the repo-local Rust toolchain (created by setup_env.ps1) into this shell.
$RootPath = $PSScriptRoot

$env:RUSTUP_HOME = "$RootPath\.rust\.rustup"
$env:CARGO_HOME = "$RootPath\.rust\.cargo"

$BinPath = "$RootPath\.rust\.cargo\bin"
if ($env:PATH -notlike "*$BinPath*") {
    $env:PATH = "$BinPath;" + $env:PATH
}

$SccacheExe = "$BinPath\sccache.exe"
if (Test-Path $SccacheExe) {
    $env:RUSTC_WRAPPER = $SccacheExe
    $env:SCCACHE_DIR = "$RootPath\.rust\.sccache"
} else {
    $env:RUSTC_WRAPPER = ""
}

Write-Host "Local Rust environment loaded." -ForegroundColor Green
Write-Host "RUSTUP_HOME:   $env:RUSTUP_HOME"
Write-Host "CARGO_HOME:    $env:CARGO_HOME"
if ($env:RUSTC_WRAPPER) { Write-Host "RUSTC_WRAPPER: $env:RUSTC_WRAPPER (sccache)" }
& "$BinPath\rustc.exe" --version
