# Build zapret-ui portable exe (and NSIS/MSI installers)
# Output:
#   src-tauri\target-dist\release\zapret-ui.exe  - built portable exe (~12 MB)
#   <project root>\zapret-ui.exe                 - ready-to-use copy in the project root
#   src-tauri\target-dist\release\bundle\nsis\*.exe - installer
#   src-tauri\target-dist\release\bundle\msi\*.msi  - MSI installer

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$env:CARGO_TARGET_DIR = Join-Path $root "src-tauri\target-dist"
$builtExe = Join-Path $env:CARGO_TARGET_DIR "release\zapret-ui.exe"
$rootExe = Join-Path $root "zapret-ui.exe"

Write-Host "Building frontend + Rust backend + bundle..."
Push-Location $root
try {
    # npm logs "Info" lines to stderr; under $ErrorActionPreference = "Stop" they
    # become a terminating NativeCommandError in PowerShell 5.1. Temporarily lower
    # EAP so stderr is non-fatal; the real result is read from $LASTEXITCODE.
    $oldEAP = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    & $env:ComSpec /c "npm run tauri build"
    $buildCode = $LASTEXITCODE
    $ErrorActionPreference = $oldEAP
    if ($buildCode -ne 0) {
        throw "tauri build failed with exit code $buildCode"
    }
} finally {
    Pop-Location
}

# Copy the ready exe into the project root for easy access.
Write-Host ""
Write-Host "Copying exe to project root..."
try {
    Copy-Item -LiteralPath $builtExe -Destination $rootExe -Force
} catch {
    throw "Could not copy exe to project root: $($_.Exception.Message). Close zapret-ui first and rebuild."
}

Write-Host ""
Write-Host "Done. Portable exe:"
Write-Host "  $rootExe"
