# Builds the main app in release mode (no bundling) and zips the exe into
# installer-app/src-tauri/payload.zip, which the installer embeds at compile time
# via `include_bytes!` (see src-tauri/src/payload.rs). Run this before building
# installer-app itself, and re-run it any time the main app changes.

$ErrorActionPreference = "Stop"

$root = Resolve-Path "$PSScriptRoot/../.."
$mainSrcTauri = Join-Path $root "src-tauri"
$installerSrcTauri = Join-Path $root "installer-app/src-tauri"

Write-Host "Building main app (release, no bundle)..."
Push-Location $root
bun run tauri build --no-bundle
Pop-Location

$exePath = Join-Path $mainSrcTauri "target/release/nectar.exe"
if (-not (Test-Path $exePath)) {
    throw "Expected build output not found at $exePath"
}

$payloadZip = Join-Path $installerSrcTauri "payload.zip"
if (Test-Path $payloadZip) { Remove-Item $payloadZip }

Write-Host "Zipping payload..."
Compress-Archive -Path $exePath -DestinationPath $payloadZip

$sizeMb = [Math]::Round((Get-Item $payloadZip).Length / 1MB, 1)
Write-Host "Payload written to $payloadZip ($sizeMb MB)"
Write-Host ""
Write-Host "Now build the installer itself:"
Write-Host "  cd installer-app; bun install; bun run tauri build --no-bundle"
