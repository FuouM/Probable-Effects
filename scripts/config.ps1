# Machine-specific paths for scripts\*.ps1, read from env.yaml at the repo root (git-ignored,
# written by setup_env.ps1, documented in env.example.yaml). Dot-source, then call
# Get-ProbableConfig. The file is flat YAML: one `key: value` per line, `#` comments, optional
# quotes. Relative paths are relative to the repo root.

$ProbableConfigKeys = @(
    # Folder holding AfterFX.exe and aerender.exe.
    "ae_support_dir",
    # Where ae-install.ps1 copies .aex files (a folder under Adobe's MediaCore plug-ins folder).
    "plugin_dir",
    # Default still for ae-smoke.ps1.
    "smoke_image"
)

function Read-FlatYaml([string]$Path) {
    $map = @{}
    $n = 0
    foreach ($line in Get-Content $Path) {
        $n++
        $text = ($line -replace '\s+#.*$', '').Trim()
        if (-not $text -or $text.StartsWith('#')) { continue }
        if ($text -notmatch '^([A-Za-z0-9_]+)\s*:\s*(.*)$') { throw "${Path}:${n}: expected 'key: value', got '$line'" }
        $value = $Matches[2].Trim()
        if ($value -match '^"(.*)"$' -or $value -match "^'(.*)'$") { $value = $Matches[1] }
        $map[$Matches[1]] = $value
    }
    $map
}

function Get-ProbableConfig {
    $root = Split-Path $PSScriptRoot -Parent
    $path = Join-Path $root "env.yaml"
    if (-not (Test-Path $path)) { throw "env.yaml missing: run .\setup_env.ps1 (or copy env.example.yaml to env.yaml and edit it)" }
    $raw = Read-FlatYaml $path
    $config = @{}
    foreach ($key in $ProbableConfigKeys) {
        if (-not $raw.ContainsKey($key) -or -not $raw[$key]) { throw "env.yaml: '$key' is missing (see env.example.yaml)" }
        $value = $raw[$key] -replace '/', '\'
        $config[$key] = if ([IO.Path]::IsPathRooted($value)) { $value } else { Join-Path $root $value }
    }
    foreach ($unknown in $raw.Keys | Where-Object { $ProbableConfigKeys -notcontains $_ }) {
        throw "env.yaml: unknown key '$unknown' (see env.example.yaml)"
    }
    $config
}
