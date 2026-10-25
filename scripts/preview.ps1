# Browser preview with shader hot reload: builds the CLI, serves http://127.0.0.1:7878/ and opens
# it. Edit any crates\effects\src\shaders\*.wgsl (or the page, crates\cli\src\serve.html) and the
# open page updates; shader compile errors show on the page. Drop an image on the page to
# change the source. Ctrl+C stops the server.
#   .\scripts\preview.ps1 [image] [-Port 7878] [-NoOpen]
param([string]$Image = "", [int]$Port = 7878, [switch]$NoOpen)
$ErrorActionPreference = "Stop"
$Root = Split-Path $PSScriptRoot -Parent
. "$Root\env.ps1" | Out-Null
Set-Location $Root

cargo build -p probable-cli
if ($LASTEXITCODE -ne 0) { throw "build failed" }
$serveArgs = @("serve", "--watch", "--port", $Port)
if (-not $NoOpen) { $serveArgs += "--open" }
if ($Image) { $serveArgs += $Image }
& "$Root\target\debug\probable.exe" @serveArgs
