<#
Installs VaultZip silently, checks the files and registry entries, then
uninstalls and checks everything was removed. Used by CI; safe to run locally
(it only touches the current user and a temporary folder).

  .\installer\smoke-test.ps1 -Installer dist\VaultZip-Setup-0.1.0.exe
#>
param([Parameter(Mandatory = $true)][string]$Installer)
$ErrorActionPreference = "Stop"

$installer = (Resolve-Path $Installer).Path
$dir = Join-Path $env:TEMP "vaultzip-smoke"
if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }

$menuKeys = @(
    "Registry::HKEY_CURRENT_USER\Software\Classes\*\shell\VaultZip",
    "Registry::HKEY_CURRENT_USER\Software\Classes\Directory\shell\VaultZip",
    "Registry::HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.zip\shell\VaultZip",
    "Registry::HKEY_CURRENT_USER\Software\Classes\VaultZip.Archive"
)
$openWithKey = "Registry::HKEY_CURRENT_USER\Software\Classes\.zip\OpenWithProgids"

function Assert($condition, $message) {
    if (-not $condition) { throw "SMOKE TEST FAILED: $message" }
    Write-Host "ok: $message"
}

Write-Host "Installing to $dir"
Start-Process -FilePath $installer -Wait -ArgumentList @(
    "/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART",
    "/DIR=`"$dir`"", "/TASKS=`"contextmenu,openwith`""
)

Assert (Test-Path "$dir\vaultzip-gui.exe") "desktop app installed"
Assert (Test-Path "$dir\vaultzip.exe") "command line tool installed"
$version = & "$dir\vaultzip.exe" --version
Assert ($version -match "vaultzip") "command line tool runs ($version)"

foreach ($key in $menuKeys) {
    Assert (Test-Path -LiteralPath $key) "registry key exists: $key"
}
$addCommand = (Get-Item -LiteralPath "$($menuKeys[0])\shell\01add\command").GetValue("")
Assert ($addCommand -like "*vaultzip-gui.exe*--add*%1*") "add command is correct ($addCommand)"
$zipCommand = (Get-Item -LiteralPath "$($menuKeys[2])\shell\02here\command").GetValue("")
Assert ($zipCommand -like "*--extract-here*") "extract here command is correct ($zipCommand)"
Assert ($null -ne (Get-Item -LiteralPath $openWithKey).GetValue("VaultZip.Archive", $null)) "Open with entry registered"

Write-Host "Uninstalling"
$uninstaller = Get-ChildItem $dir -Filter "unins*.exe" | Select-Object -First 1
Assert ($null -ne $uninstaller) "uninstaller present"
Start-Process -FilePath $uninstaller.FullName -Wait -ArgumentList @("/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART")
# The uninstaller restarts itself from a temporary folder, so wait for it to finish.
for ($i = 0; $i -lt 60 -and (Test-Path "$dir\vaultzip-gui.exe"); $i++) { Start-Sleep -Seconds 1 }

Assert (-not (Test-Path "$dir\vaultzip-gui.exe")) "application files removed"
foreach ($key in $menuKeys) {
    Assert (-not (Test-Path -LiteralPath $key)) "registry key removed: $key"
}
$remaining = (Get-Item -LiteralPath $openWithKey -ErrorAction SilentlyContinue)
Assert (($null -eq $remaining) -or ($null -eq $remaining.GetValue("VaultZip.Archive", $null))) "Open with entry removed"
Write-Host "Smoke test passed."
