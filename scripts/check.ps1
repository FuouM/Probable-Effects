# One command for the whole verification loop.
#   .\scripts\check.ps1        fmt check, clippy, workspace tests
#   .\scripts\check.ps1 -Ae    also build + install the AE plugins and run the AE smoke test
#                              (close After Effects first)
param([switch]$Ae, [switch]$Release)
$ErrorActionPreference = "Stop"
$Root = Split-Path $PSScriptRoot -Parent
. "$Root\env.ps1" | Out-Null
Set-Location $Root

function Step($name, [scriptblock]$run) {
    Write-Host "== $name" -ForegroundColor Cyan
    $sw = [Diagnostics.Stopwatch]::StartNew()
    & $run
    if ($LASTEXITCODE -ne 0) { throw "$name failed (exit $LASTEXITCODE)" }
    Write-Host ("   {0:n1}s" -f $sw.Elapsed.TotalSeconds)
}

Step "cargo fmt" { cargo fmt --all -- --check }
Step "clippy" { cargo clippy --workspace --all-targets -- -D warnings }
Step "tests" { cargo test --workspace }
if ($Ae) {
    Step "cli build" { cargo build -p probable-cli }
    $flag = if ($Release) { @("-Release") } else { @() }
    & "$PSScriptRoot\ae-build.ps1" @flag
    & "$PSScriptRoot\ae-install.ps1" @flag
    & "$PSScriptRoot\ae-smoke.ps1"
}
Write-Host "all checks passed" -ForegroundColor Green
