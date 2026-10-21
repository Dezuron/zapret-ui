# Ensures the app version is identical in package.json, Cargo.toml and
# tauri.conf.json. Run in CI and before a release.
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$packageVersion = (Get-Content (Join-Path $root "package.json") -Raw | ConvertFrom-Json).version
$cargoText = Get-Content (Join-Path $root "src-tauri\Cargo.toml") -Raw
$cargoMatch = [regex]::Match($cargoText, '(?m)^version\s*=\s*"([^"]+)"')
$tauriVersion = (Get-Content (Join-Path $root "src-tauri\tauri.conf.json") -Raw | ConvertFrom-Json).version

if (-not $packageVersion) {
    throw "package.json has no version"
}
if (-not $cargoMatch.Success) {
    throw "src-tauri/Cargo.toml has no [package] version"
}

$cargoVersion = $cargoMatch.Groups[1].Value
$versions = @($packageVersion, $cargoVersion, $tauriVersion) | Select-Object -Unique
if ($versions.Count -ne 1) {
    throw "Version mismatch: package.json=$packageVersion, Cargo.toml=$cargoVersion, tauri.conf.json=$tauriVersion"
}

Write-Host "Version $packageVersion is consistent across package.json, Cargo.toml and tauri.conf.json."
