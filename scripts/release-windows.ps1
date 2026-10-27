# Builds the release `dail.exe` and packages it as dist/dail-<version>-x86_64-pc-windows-msvc.zip and
# dist/dail-<version>-x86_64.msi (+ .sha256 for each). The MSI needs WiX v5: `dotnet tool install --global wix --version 5.0.2`.
# Run from anywhere: `pwsh scripts/release-windows.ps1`. The release workflow (.github/workflows/release.yml) runs it too.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true

$root = Split-Path $PSScriptRoot
Set-Location $root

# `cargo pkgid -p app-cli` prints `path+file:///…/crates/app-cli#app-cli@0.1.0`.
$version = (cargo pkgid -p app-cli) -replace '^.*[#@]', ''
$target = 'x86_64-pc-windows-msvc'
$name = "dail-$version-$target"

# crates/app-cli/build.rs embeds packaging/windows/dail.manifest; [profile.release] does thin LTO and strips.
cargo build --release --locked -p app-cli

$dist = Join-Path $root 'dist'
$stage = Join-Path $dist $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $root 'target/release/dail.exe') $stage

$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
Remove-Item -Recurse -Force $stage

# MSI (Wave 7 R1): packaging/windows/dail.wxs, a per-machine install with a Start menu shortcut.
$msi = Join-Path $dist "dail-$version-x86_64.msi"
if (Test-Path $msi) { Remove-Item -Force $msi }
wix build -arch x64 -d "Version=$version" -d "Exe=$(Join-Path $root 'target/release/dail.exe')" -o $msi `
    (Join-Path $root 'packaging/windows/dail.wxs')
Remove-Item -Force -ErrorAction SilentlyContinue (Join-Path $dist "dail-$version-x86_64.wixpdb")

foreach ($file in @($zip, $msi)) {
    $hash = (Get-FileHash -Algorithm SHA256 $file).Hash.ToLowerInvariant()
    Set-Content -NoNewline -Path "$file.sha256" -Value "$hash  $(Split-Path -Leaf $file)`n"
    Write-Output "built $file"
}
