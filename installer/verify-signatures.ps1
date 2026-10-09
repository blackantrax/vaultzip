<#
Fails unless every given file has a valid, timestamped Authenticode signature.

  .\installer\verify-signatures.ps1 -Paths dist\VaultZip-Setup-0.1.0.exe
#>
param([Parameter(Mandatory = $true)][string[]]$Paths)
$ErrorActionPreference = "Stop"

foreach ($path in $Paths) {
    $sig = Get-AuthenticodeSignature -FilePath $path
    if ($sig.Status -ne "Valid") {
        throw "Signature check failed for ${path}: status is $($sig.Status)"
    }
    if ($null -eq $sig.TimeStamperCertificate) {
        throw "Signature on ${path} has no timestamp"
    }
    Write-Host "ok: ${path}"
    Write-Host "    signed by $($sig.SignerCertificate.Subject)"
}
Write-Host "All signatures are valid."
