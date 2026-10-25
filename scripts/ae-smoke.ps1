# End-to-end check of the installed plugins inside After Effects, without touching the AE UI:
#   1. AfterFX.exe -r scripts\ae-smoke.jsx builds one project per bit depth (8/16/32 bpc), each
#      with one comp per case in tests\ae-smoke-cases.ps1. Only when the cases or image changed, or
#      with -Regenerate.
#   2. aerender renders each project headless (three starts however many effects there are; they
#      run one after another because concurrent aerender starts contend and are slower).
#   3. The probable CLI renders every case; `probable diff` compares each AE frame against it.
# Run scripts\ae-build.ps1 and scripts\ae-install.ps1 first.
param([string]$Image = "", [switch]$Regenerate)
$ErrorActionPreference = "Stop"
$Root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot\config.ps1"
$config = Get-ProbableConfig

$afterFx = Join-Path $config.ae_support_dir "AfterFX.exe"
$aerender = Join-Path $config.ae_support_dir "aerender.exe"
foreach ($exe in $afterFx, $aerender) {
    if (-not (Test-Path $exe)) { throw "$exe not found: check ae_support_dir in env.yaml" }
}
$cli = Join-Path $Root "target\debug\probable.exe"
if (-not (Test-Path $cli)) { throw "build the CLI first: cargo build -p probable-cli" }

if (-not $Image) { $Image = $config.smoke_image }
$Image = (Resolve-Path $Image).Path
$cases = @(& "$Root\tests\ae-smoke-cases.ps1")
$dir = Join-Path $Root "target\smoke\ae"
New-Item -ItemType Directory -Force -Path $dir | Out-Null

# The projects depend on the cases and the image; regenerate when either changed.
$caseLines = foreach ($c in $cases) {
    $params = ($c.AeParams.GetEnumerator() | Sort-Object Key | ForEach-Object { "$($_.Key)=$($_.Value)" }) -join ";"
    "$($c.Name)`t$($c.MatchName)`t$params"
}
$stamp = (@("image`t$Image") + $caseLines) -join "`n"
$depths = 8, 16, 32
$stale = $Regenerate -or -not (Test-Path "$dir\stamp.txt") -or ((Get-Content "$dir\stamp.txt" -Raw) -ne $stamp) -or
    ($depths | Where-Object { -not (Test-Path "$dir\smoke_$_.aep") })
if ($stale) {
    if (Get-Process AfterFX -ErrorAction SilentlyContinue) { throw "close After Effects first" }
    Remove-Item "$dir\smoke_*.aep", "$dir\ae-smoke.log", "$dir\stamp.txt" -ErrorAction SilentlyContinue
    $caseLines | Set-Content "$dir\cases.txt"
    $env:PROBABLE_SMOKE_DIR = $dir -replace '\\', '/'
    $env:PROBABLE_SMOKE_IMAGE = $Image -replace '\\', '/'
    Write-Host "generating projects with $afterFx"
    # Start-Process goes through ShellExecute, whose AfterFX.exe returns before the script runs;
    # a direct CreateProcess launch is the process that runs -r and quits. AfterFX takes quotes
    # around the -r path literally, so pass the space-free 8.3 short path unquoted.
    $script = (New-Object -ComObject Scripting.FileSystemObject).GetFile("$PSScriptRoot\ae-smoke.jsx").ShortPath
    $psi = New-Object Diagnostics.ProcessStartInfo $afterFx, "-r $script"
    $psi.UseShellExecute = $false
    $p = [Diagnostics.Process]::Start($psi)
    if (-not $p.WaitForExit(300000)) { $p.Kill(); throw "After Effects did not finish generating projects in 5 minutes" }
    foreach ($d in $depths) {
        if (-not (Test-Path "$dir\smoke_$d.aep")) {
            $log = if (Test-Path "$dir\ae-smoke.log") { Get-Content "$dir\ae-smoke.log" -Raw } else { "no ae-smoke.log: the script did not run or cannot write files (AE Preferences > Scripting & Expressions)" }
            throw "project for $d bpc was not created: $log"
        }
    }
    Set-Content "$dir\stamp.txt" $stamp -NoNewline
}

foreach ($c in $cases) {
    & $cli render $c.Name $Image "$dir\cli_$($c.Name).png" @($c.CliParams)
    if ($LASTEXITCODE -ne 0) { throw "CLI render of $($c.Name) failed" }
}

$failed = $false
foreach ($d in $depths) {
    Remove-Item "$dir\ae_${d}_*.tif" -ErrorAction SilentlyContinue
    & $aerender -project "$dir\smoke_$d.aep" -sound OFF | Out-File "$dir\aerender_$d.log"
    foreach ($c in $cases) {
        $frame = Get-ChildItem "$dir\ae_${d}_$($c.Name)_*.tif" | Select-Object -First 1
        if (-not $frame) { Write-Host "$($c.Name) $d bpc: no frame rendered, see $dir\aerender_$d.log" -ForegroundColor Red; $failed = $true; continue }
        Write-Host -NoNewline "$($c.Name) $d bpc vs CLI: "
        & $cli diff $frame.FullName "$dir\cli_$($c.Name).png"
        if ($LASTEXITCODE -ne 0) { $failed = $true }
    }
}
if ($failed) { throw "smoke test failed" }
