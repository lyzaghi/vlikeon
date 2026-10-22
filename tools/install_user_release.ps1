param(
    [Parameter(Mandatory=$true)][string]$BootstrapDirectory,
    [Parameter(Mandatory=$true)][string]$ReleaseEnvelope,
    [Parameter(Mandatory=$true)][string]$PackagePath,
    [Parameter(Mandatory=$true)][string]$PublisherCertificateSha256,
    [switch]$CreateDesktopShortcut
)
$ErrorActionPreference='Stop'
# This is an explicit per-user installer. No UAC, browser-default change or service
# registration is performed. The browser package is verified by its embedded root.
$omniBootstrap=(Resolve-Path -LiteralPath $BootstrapDirectory).Path
$omniEnvelope=(Resolve-Path -LiteralPath $ReleaseEnvelope).Path
$omniPackage=(Resolve-Path -LiteralPath $PackagePath).Path
if($PublisherCertificateSha256 -notmatch '^[0-9a-fA-F]{64}$'){throw 'An explicit SHA-256 publisher certificate pin is required.'}
$omniNames=@('omniplay-launcher.exe','omniplay-update.exe')
foreach($omniName in $omniNames){
    $omniPath=Join-Path $omniBootstrap $omniName
    $omniSignature=Get-AuthenticodeSignature -LiteralPath $omniPath
    if($omniSignature.Status -ne 'Valid' -or $null -eq $omniSignature.SignerCertificate){throw 'The bootstrap publisher signature is not valid.'}
    $omniHasher=[System.Security.Cryptography.SHA256]::Create()
    try{$omniCertificateHash=([BitConverter]::ToString($omniHasher.ComputeHash($omniSignature.SignerCertificate.RawData))).Replace('-','')}
    finally{$omniHasher.Dispose()}
    if($omniCertificateHash -ine $PublisherCertificateSha256){throw 'The bootstrap publisher does not match the selected certificate pin.'}
}
$omniInstallRoot=Join-Path $env:LOCALAPPDATA 'OmniPlay\install'
$omniBootstrapRoot=Join-Path $omniInstallRoot 'bootstrap'
if(Test-Path -LiteralPath $omniBootstrapRoot){
    # Resume only with byte-identical signed bootstrap files; never replace another build.
    foreach($omniName in $omniNames){
        $omniExistingPath=Join-Path $omniBootstrapRoot $omniName
        if(-not (Test-Path -LiteralPath $omniExistingPath)){throw 'The existing bootstrap is incomplete and requires explicit maintenance.'}
        if((Get-FileHash -LiteralPath $omniExistingPath -Algorithm SHA256).Hash -ne (Get-FileHash -LiteralPath (Join-Path $omniBootstrap $omniName) -Algorithm SHA256).Hash){throw 'A different stable bootstrap is installed. Use the signed bootstrap maintenance installer.'}
    }
}else{
    New-Item -ItemType Directory -Path $omniBootstrapRoot | Out-Null
    foreach($omniName in $omniNames){Copy-Item -LiteralPath (Join-Path $omniBootstrap $omniName) -Destination (Join-Path $omniBootstrapRoot $omniName)}
}
$omniUpdater=Join-Path $omniBootstrapRoot 'omniplay-update.exe'
$omniStatusRaw=& $omniUpdater status
if($LASTEXITCODE -ne 0){throw 'The existing installation state could not be read.'}
$omniStatus=$omniStatusRaw | ConvertFrom-Json
$omniInputEnvelope=Get-Content -LiteralPath $omniEnvelope -Raw | ConvertFrom-Json
$omniInputMetadata=[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($omniInputEnvelope.payload_base64)) | ConvertFrom-Json
# This untrusted preview only detects an already active release so interrupted
# registration can resume. Every new package still goes through signed verification.
$omniExistingActive=@($omniStatus.releases | Where-Object {$_.status -eq 'active' -and $_.version -eq $omniInputMetadata.version -and $_.metadata_version -eq $omniInputMetadata.metadata_version})
if($omniExistingActive.Count -eq 0){
    $omniStagedRaw=& $omniUpdater stage $omniEnvelope $omniPackage
    if($LASTEXITCODE -ne 0){throw 'The signed release package was not accepted.'}
    $omniStaged=$omniStagedRaw | ConvertFrom-Json
    if($omniStaged.id -notmatch '^[0-9a-f-]{36}$'){throw 'The updater returned an invalid release identity.'}
    $omniActivatedRaw=& $omniUpdater activate $omniStaged.id
    if($LASTEXITCODE -ne 0){throw 'The release could not be activated.'}
}
$omniLauncher=Join-Path $omniBootstrapRoot 'omniplay-launcher.exe'
& (Join-Path $PSScriptRoot 'install_native_host.ps1') -ExecutablePath $omniLauncher
if($CreateDesktopShortcut){
    $omniDesktop=[Environment]::GetFolderPath('Desktop')
    $omniShortcutPath=Join-Path $omniDesktop 'OmniPlay.lnk'
    if(Test-Path -LiteralPath $omniShortcutPath){throw 'The desktop shortcut already exists; the installed release remains available.'}
    $omniShell=New-Object -ComObject WScript.Shell
    $omniShortcut=$omniShell.CreateShortcut($omniShortcutPath)
    $omniShortcut.TargetPath=$omniLauncher
    $omniShortcut.WorkingDirectory=$omniBootstrapRoot
    $omniShortcut.Description='OmniPlay'
    $omniShortcut.Save()
}
Write-Output 'OmniPlay installed for the current user. Profile and save data are independent of the version directory.'
