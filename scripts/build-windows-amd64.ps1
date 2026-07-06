$ErrorActionPreference = "Stop"

$RootDir = Resolve-Path (Join-Path $PSScriptRoot "..")
$Target = "x86_64-pc-windows-msvc"

if (-not $IsWindows) {
    Write-Error "This script must run on Windows with the MSVC build tools installed."
}

Set-Location $RootDir

Write-Host "Installing Rust target: $Target"
rustup target add $Target

Write-Host "Installing frontend dependencies"
yarn install --frozen-lockfile

Write-Host "Building Windows amd64 package"
yarn tauri build --target $Target

Write-Host "Done. Artifacts:"
Write-Host "  src-tauri\target\$Target\release\bundle"
