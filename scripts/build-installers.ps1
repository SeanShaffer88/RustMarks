# Builds RustMarks-v<ver>-x64.msi and RustMarks-v<ver>-x64-Setup.exe (WiX 5, per-user, offline payload).
# Usage: pwsh scripts/build-installers.ps1 -Version 1.0.0 -OutDir dist
param([Parameter(Mandatory)][string]$Version, [string]$OutDir = "dist")
$ErrorActionPreference = "Stop"
$root = (Resolve-Path "$PSScriptRoot/..").Path
$exe = Join-Path $root "target/release/rustmarks.exe"
if (-not (Test-Path $exe)) { throw "Build first: cargo build --release --locked" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$out = (Resolve-Path $OutDir).Path
$wixVer = "5.0.2"
dotnet tool install --global wix --version $wixVer 2>&1 | Out-Host
$env:PATH += ";$env:USERPROFILE\.dotnet\tools"
wix extension add -g "WixToolset.BootstrapperApplications.wixext/$wixVer"
if ($LASTEXITCODE -ne 0) { throw "wix extension add failed" }
$msi = Join-Path $out "RustMarks-v$Version-x64.msi"
$setup = Join-Path $out "RustMarks-v$Version-x64-Setup.exe"
wix build "$root/packaging/rustmarks.wxs" -arch x64 -d "Version=$Version" -d "ExePath=$exe" -d "RepoRoot=$root" -o $msi
if ($LASTEXITCODE -ne 0) { throw "MSI build failed" }
wix build "$root/packaging/bundle.wxs" -arch x64 -ext WixToolset.BootstrapperApplications.wixext -d "Version=$Version" -d "MsiPath=$msi" -o $setup
if ($LASTEXITCODE -ne 0) { throw "Setup build failed" }
Get-Item $msi, $setup | Format-Table Name, Length
