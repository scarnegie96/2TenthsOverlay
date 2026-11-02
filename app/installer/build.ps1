# Builds the installer: dist\Tenths_<version>_x64-setup.exe, from a release build of tenths.exe.
#
#   cargo build --release -p tenths
#   powershell -File app\installer\build.ps1
#
# The version is app\Cargo.toml's unless -Version says otherwise. Needs NSIS
# (makensis); it's on GitHub's Windows runners, and is installed with Chocolatey
# when it's missing. The workflows (build.yml, release.yml) run this script.
param(
    [string]$Version,
    [string]$Exe = "target\release\tenths.exe",
    [string]$OutDir = "dist"
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Set-Location $root

if (-not $Version) {
    $line = Select-String -Path "app\Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
    if (-not $line) { throw "No version in app\Cargo.toml" }
    $Version = $line.Matches[0].Groups[1].Value
}
if (-not (Test-Path $Exe)) { throw "$Exe isn't there: run cargo build --release -p tenths first" }

$makensis = (Get-Command makensis -ErrorAction SilentlyContinue).Source
if (-not $makensis) {
    foreach ($dir in "${env:ProgramFiles(x86)}\NSIS", "$env:ProgramFiles\NSIS") {
        if (Test-Path "$dir\makensis.exe") { $makensis = "$dir\makensis.exe"; break }
    }
}
if (-not $makensis) {
    choco install nsis -y --no-progress
    $makensis = "${env:ProgramFiles(x86)}\NSIS\makensis.exe"
}

# The four-number file version Windows wants: 0.5.0 -> 0.5.0.0 (a pre-release suffix is dropped).
$num = ($Version -split '-')[0] + ".0"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$out = Join-Path (Resolve-Path $OutDir).Path "Tenths_${Version}_x64-setup.exe"

& $makensis /V2 "/DVERSION=$Version" "/DVERSION_NUM=$num" "/DEXE=$((Resolve-Path $Exe).Path)" "/DICON=$root\app\assets\icons\icon.ico" "/DOUTFILE=$out" "app\installer\tenths.nsi"
if ($LASTEXITCODE -ne 0) { throw "makensis failed ($LASTEXITCODE)" }
Write-Host "Built $out"
