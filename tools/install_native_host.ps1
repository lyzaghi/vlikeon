param(
    [Parameter(Mandatory=$true)][string]$ExecutablePath,
    [switch]$Development
)
$ErrorActionPreference='Stop'
$omniRepositoryRoot=Split-Path -Parent $PSScriptRoot
$omniExecutable=(Resolve-Path -LiteralPath $ExecutablePath).Path
if ([System.IO.Path]::GetFileName($omniExecutable) -notin @('omniplay-broker.exe','omniplay-launcher.exe')) {throw 'Choose the OmniPlay broker or stable launcher executable.'}
if (-not $Development) {
    $omniSignature=Get-AuthenticodeSignature -LiteralPath $omniExecutable
    if ($omniSignature.Status -ne 'Valid') {throw 'A production host must carry a valid Authenticode signature.'}
}
$omniIdentity=Get-Content -LiteralPath (Join-Path $omniRepositoryRoot 'security\component_identity.json') -Raw | ConvertFrom-Json
if ($omniIdentity.origin -notmatch '^chrome-extension://[a-p]{32}/$') {throw 'The component origin is invalid.'}
$omniManifestDirectory=Join-Path ([System.IO.Path]::GetDirectoryName($omniExecutable)) 'native-messaging'
New-Item -ItemType Directory -Path $omniManifestDirectory -Force | Out-Null
$omniManifestPath=Join-Path $omniManifestDirectory 'com.omniplay.broker.json'
$omniManifest=[ordered]@{
    name='com.omniplay.broker'
    description='OmniPlay local game library and progress service'
    path=$omniExecutable
    type='stdio'
    allowed_origins=@($omniIdentity.origin)
}
$omniUtf8=[System.Text.UTF8Encoding]::new($false)
[System.IO.File]::WriteAllText($omniManifestPath,($omniManifest | ConvertTo-Json -Depth 5),$omniUtf8)
# Register only for OmniPlay. This does not modify Chrome/Chromium profiles or
# browser defaults and does not grant the host to arbitrary website origins.
$omniRegistryPath='HKCU:\SOFTWARE\OmniPlay\NativeMessagingHosts\com.omniplay.broker'
New-Item -Path $omniRegistryPath -Force | Out-Null
Set-Item -LiteralPath $omniRegistryPath -Value $omniManifestPath
Write-Output 'OmniPlay native messaging host registered for the current user.'
