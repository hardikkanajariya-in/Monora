$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$release = Join-Path $root "src-tauri\target\release"
$out = Join-Path $root "dist-portable\SimpleRecorder"

$exe = Get-ChildItem -Path $release -Filter "*.exe" -File |
    Where-Object { $_.Name -notmatch 'wix|msi|setup' } |
    Select-Object -First 1

if (-not $exe) {
    Write-Error "Release binary not found. Run: npm run tauri build"
}

New-Item -ItemType Directory -Force -Path $out, (Join-Path $out "config"), (Join-Path $out "logs") | Out-Null

Copy-Item (Join-Path $release "*.exe") $out -Force
Copy-Item (Join-Path $release "*.dll") $out -Force -ErrorAction SilentlyContinue

$zip = Join-Path $root "dist-portable\SimpleRecorder-portable.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $root "dist-portable\SimpleRecorder") -DestinationPath $zip -Force
Write-Host "Portable package: $zip"
