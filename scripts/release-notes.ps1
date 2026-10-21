param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Output = "release-notes.md"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$changelog = Join-Path $root "CHANGELOG.md"
$lines = Get-Content -Path $changelog -Encoding UTF8
$start = -1

for ($i = 0; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match "^## \[$([regex]::Escape($Version))\]") {
        $start = $i
        break
    }
}
if ($start -lt 0) {
    throw "CHANGELOG.md has no section for version $Version"
}

$end = $lines.Count
for ($i = $start + 1; $i -lt $lines.Count; $i++) {
    if ($lines[$i] -match "^## \[") {
        $end = $i
        break
    }
}

$section = ($lines[$start..($end - 1)] -join "`n").TrimEnd() + "`n"
$outPath = if ([IO.Path]::IsPathRooted($Output)) { $Output } else { Join-Path (Get-Location) $Output }
[IO.File]::WriteAllText($outPath, $section, (New-Object Text.UTF8Encoding($false)))
Write-Host "Release notes for $Version written to $outPath"
