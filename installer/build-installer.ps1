<#
Builds the VaultZip Windows installer with Inno Setup.

Run from the repository root after "cargo build --release":
  .\installer\build-installer.ps1 -Version 0.1.0 -BuildDir target\release

The installer is written to the output folder (default: dist).
#>
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$BuildDir,
    [string]$OutDir = "dist"
)
$ErrorActionPreference = "Stop"

function Find-Iscc {
    $cmd = Get-Command iscc.exe -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    )
    return $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}

$iscc = Find-Iscc
if (-not $iscc) {
    if (Get-Command choco -ErrorAction SilentlyContinue) {
        choco install innosetup -y --no-progress | Out-Host
        $iscc = Find-Iscc
    }
}
if (-not $iscc) {
    throw "Inno Setup 6 was not found. Install it from https://jrsoftware.org/isinfo.php and run again."
}

foreach ($exe in @("vaultzip-gui.exe", "vaultzip.exe")) {
    if (-not (Test-Path (Join-Path $BuildDir $exe))) {
        throw "Missing $exe in $BuildDir. Run 'cargo build --release' first."
    }
}

$build = (Resolve-Path $BuildDir).Path
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$out = (Resolve-Path $OutDir).Path

& $iscc "/DAppVersion=$Version" "/DBuildDir=$build" "/O$out" "installer\vaultzip.iss"
if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed with exit code $LASTEXITCODE" }
Write-Host "Installer written to $out"
