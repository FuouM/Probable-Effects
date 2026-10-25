# Install staged .aex files into the plugin folder from env.yaml (`plugin_dir`, under Adobe's
# shared MediaCore folder, so AE and Premiere both load them).
#   .\scripts\ae-install.ps1 -Setup     once, from an elevated PowerShell: creates plugin_dir and
#                                      gives your user write access to it
#   .\scripts\ae-install.ps1 [-Release] copy target\<profile>\aex\*.aex there (no elevation)
# Close After Effects first: it locks loaded plugins.
param([switch]$Setup, [switch]$Release, [string]$User = "$env:USERDOMAIN\$env:USERNAME")
$ErrorActionPreference = "Stop"
$Root = Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot\config.ps1"
$Dest = (Get-ProbableConfig).plugin_dir

if ($Setup) {
    $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
        [Security.Principal.WindowsBuiltInRole]::Administrator)
    if (-not $admin) { throw "-Setup must run in an elevated PowerShell" }
    New-Item -ItemType Directory -Force -Path $Dest | Out-Null
    $acl = Get-Acl $Dest
    $rule = New-Object Security.AccessControl.FileSystemAccessRule($User, "Modify", "ContainerInherit,ObjectInherit", "None", "Allow")
    $acl.AddAccessRule($rule)
    Set-Acl $Dest $acl
    Write-Host "$Dest is writable by $User"
    return
}

if (-not (Test-Path $Dest)) { throw "$Dest does not exist: run '.\scripts\ae-install.ps1 -Setup' once from an elevated PowerShell" }
if (Get-Process AfterFX, "Adobe Premiere Pro" -ErrorAction SilentlyContinue) { throw "close After Effects / Premiere first (they lock loaded plugins)" }

$profileDir = if ($Release) { "release" } else { "debug" }
$staged = Get-ChildItem "$Root\target\$profileDir\aex\*.aex"
if (-not $staged) { throw "nothing staged in target\$profileDir\aex: run .\scripts\ae-build.ps1 first" }
foreach ($f in $staged) {
    Copy-Item -Force $f.FullName $Dest
    Write-Host "installed $Dest\$($f.Name)"
}
