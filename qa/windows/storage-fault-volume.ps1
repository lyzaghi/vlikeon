param(
    [Parameter(Mandatory=$true)][ValidateSet('Prepare','Detach')][string]$Mode,
    [Parameter(Mandatory=$true)][string]$Workspace
)
$ErrorActionPreference='Stop'
# This helper manages only the newly created 256-MiB image in this task's
# explicitly named QA directory. It never selects a host disk by number.
$expected='E:\V10\work\vlikeon-items-50-109-01a16780\full-disk-052'
$root=[IO.Path]::GetFullPath($Workspace).TrimEnd('\')
if($root -cne $expected){throw 'Unexpected volume workspace'}
$principal=[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent())
if(-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)){throw 'Windows administrative confirmation is required to attach this private test image'}
$parent=Split-Path -Parent $root
if(-not(Test-Path -LiteralPath $parent -PathType Container)){throw 'QA parent must already exist'}
for($current=Get-Item -LiteralPath $parent; $null -ne $current; $current=$current.Parent){if($current.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Linked workspace ancestor'}}
$image=Join-Path $root 'vlikeon-storage-052.vhdx'
$mount=Join-Path $root 'volume'
$receipt=Join-Path $root 'volume.json'
if($Mode -eq 'Detach'){
    if(-not(Test-Path -LiteralPath $receipt -PathType Leaf)){throw 'Missing owned image receipt'}
    $owned=Get-Content -LiteralPath $receipt -Raw | ConvertFrom-Json
    if($owned.image -cne $image -or $owned.mount -cne $mount -or $owned.maximum_bytes -ne 268435456){throw 'Invalid image receipt'}
    $diskImage=Get-DiskImage -ImagePath $image
    if($diskImage.Attached){Dismount-DiskImage -ImagePath $image -ErrorAction Stop}
    $owned | Add-Member -NotePropertyName detached_at -NotePropertyValue ([DateTime]::UtcNow.ToString('o')) -Force
    $owned | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $receipt -Encoding utf8
    Write-Output 'Detached the owned QA image; image and fixture data are retained.'
    exit
}
if(Test-Path -LiteralPath $root){throw 'Fresh volume workspace required; do not overwrite a prior or uncertain run'}
New-Item -ItemType Directory -Path $root | Out-Null
New-Item -ItemType Directory -Path $mount | Out-Null
$script=Join-Path $root 'create-image.diskpart'
@("create vdisk file=`"$image`" maximum=256 type=expandable","select vdisk file=`"$image`"",'attach vdisk','exit') | Set-Content -LiteralPath $script -Encoding ascii
$attached=$false
try{
    & "$env:SystemRoot\System32\diskpart.exe" /s $script | Set-Content -LiteralPath (Join-Path $root 'diskpart.log') -Encoding utf8
    if($LASTEXITCODE -ne 0){throw 'Disk image creation/attachment failed; inspect diskpart.log'}
    $diskImage=Get-DiskImage -ImagePath $image -ErrorAction Stop
    if(-not $diskImage.Attached){throw 'QA image is not attached'}
    $attached=$true
    $disks=@($diskImage | Get-Disk)
    if($disks.Count -ne 1){throw 'Image did not resolve to exactly one disk'}
    $disk=$disks[0]
    if($disk.Size -ne 268435456 -or $disk.PartitionStyle -ne 'RAW' -or $disk.IsBoot -or $disk.IsSystem){throw 'Refusing to initialize anything except this new raw 256-MiB image'}
    $disk | Initialize-Disk -PartitionStyle GPT -PassThru | Out-Null
    $partition=New-Partition -DiskNumber $disk.Number -UseMaximumSize
    $volume=Format-Volume -Partition $partition -FileSystem NTFS -NewFileSystemLabel 'VLIKEON_QA052' -Confirm:$false
    if($volume.FileSystem -ne 'NTFS' -or $volume.Size -gt 268435456 -or $volume.FileSystemLabel -ne 'VLIKEON_QA052'){throw 'Unexpected test filesystem'}
    Add-PartitionAccessPath -DiskNumber $disk.Number -PartitionNumber $partition.PartitionNumber -AccessPath ($mount+'\')
    [ordered]@{schema_version=1;image=$image;mount=$mount;maximum_bytes=268435456;disk_number=$disk.Number;partition_number=$partition.PartitionNumber;volume_unique_id=$volume.UniqueId;prepared_at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $receipt -Encoding utf8
    Write-Output "Prepared the isolated 256-MiB QA volume at $mount"
}catch{
    [ordered]@{message=$_.Exception.Message;failed_at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $root 'prepare-error.json') -Encoding utf8
    if($attached){Dismount-DiskImage -ImagePath $image -ErrorAction SilentlyContinue}
    throw
}
