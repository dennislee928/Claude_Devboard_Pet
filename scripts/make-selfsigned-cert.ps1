# make-selfsigned-cert.ps1 — create (or reuse) a self-signed code-signing
# certificate for DevPet and return it.
#
# A self-signed certificate does not make Windows trust DevPet automatically;
# it gives the binaries a verifiable publisher identity and a signature, and
# SmartScreen shows "DevPet Project" instead of "Unknown publisher" once the
# exported .cer has been imported into Trusted Root / Trusted Publishers.
#
#   $cert = .\make-selfsigned-cert.ps1 -Export dist\DevPet-selfsigned.cer
param(
    [string]$Subject = 'CN=DevPet Project, O=DevPet Project, C=TW',
    [string]$FriendlyName = 'DevPet Project (self-signed)',
    [string]$Export,
    [string]$ExportPfx,
    [string]$PfxPassword = 'devpet',
    [int]$Years = 5
)

$ErrorActionPreference = 'Stop'

$existing = Get-ChildItem Cert:\CurrentUser\My -CodeSigningCert -ErrorAction SilentlyContinue |
    Where-Object { $_.Subject -eq $Subject -and $_.NotAfter -gt (Get-Date).AddDays(30) } |
    Sort-Object NotAfter -Descending | Select-Object -First 1

if ($existing) {
    Write-Host "reusing certificate $($existing.Thumbprint)"
    $cert = $existing
} else {
    $cert = New-SelfSignedCertificate `
        -Type CodeSigningCert `
        -Subject $Subject `
        -FriendlyName $FriendlyName `
        -KeyUsage DigitalSignature `
        -KeyAlgorithm RSA -KeyLength 2048 `
        -CertStoreLocation Cert:\CurrentUser\My `
        -NotAfter (Get-Date).AddYears($Years) `
        -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3')
    Write-Host "created certificate $($cert.Thumbprint)"
}

if ($Export) {
    New-Item -ItemType Directory -Force (Split-Path $Export) | Out-Null
    Export-Certificate -Cert $cert -FilePath $Export -Force | Out-Null
    Write-Host "public certificate -> $Export"
}
if ($ExportPfx) {
    New-Item -ItemType Directory -Force (Split-Path $ExportPfx) | Out-Null
    $pw = ConvertTo-SecureString -String $PfxPassword -Force -AsPlainText
    Export-PfxCertificate -Cert $cert -FilePath $ExportPfx -Password $pw | Out-Null
    Write-Host "pfx -> $ExportPfx"
}

$cert
