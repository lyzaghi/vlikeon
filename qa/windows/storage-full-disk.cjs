// Destructive only to a newly created filler on the explicitly owned QA VHDX.
// Refuse an unmounted host folder before every bounded write. Never resize a disk.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict'),{spawnSync}=require('node:child_process');
const expected='E:\\V10\\work\\vlikeon-items-50-109-01a16780\\full-disk-052',maximum=256n*1024n**2n;
const output=process.env.VLIKEON_QA_OUTPUT,exe=process.env.VLIKEON_STORAGE_EXE,root=process.env.VLIKEON_STORAGE_VOLUME;
const report={item:52,scope:'Actual NTFS disk-full on private 256-MiB VHDX; recovery and retry',status:'running',cases:[]};
let serial=0,fillerCreated=false,reserveCreated=false,filler,reserve,mount;
function capacity(){const info=fs.statfsSync(mount,{bigint:true}),bytes=info.bsize*info.blocks;assert.ok(bytes>16n*1024n**2n&&bytes<=maximum,'Mount is not the private small volume');return{total_bytes:Number(bytes),available_bytes:Number(info.bsize*info.bavail)};}
function identity(){
  const escaped=root.replaceAll("'","''"),command=`$ErrorActionPreference='Stop'; $r=Get-Content -LiteralPath '${escaped}\\volume.json' -Raw | ConvertFrom-Json; $image=Get-DiskImage -ImagePath $r.image; if(-not $image.Attached){throw 'Image not attached'}; $expectedDevice=[string]::Concat([char]92,[char]92,'.',[char]92,'PhysicalDrive',$r.disk_number); if($image.DevicePath -cne $expectedDevice -or $image.Size -ne 268435456){throw 'Image device identity mismatch'}; $disks=@(Get-Disk -Number $r.disk_number); if($disks.Count -ne 1 -or $disks[0].Number -ne $r.disk_number -or $disks[0].Size -ne 268435456 -or $disks[0].IsBoot -or $disks[0].IsSystem){throw 'Unexpected image disk'}; $p=Get-Partition -DiskNumber $r.disk_number -PartitionNumber $r.partition_number; if($p.AccessPaths -notcontains ($r.mount+'\\')){throw 'Owned mount missing'}; $v=Get-Volume -Partition $p; if($v.UniqueId -cne $r.volume_unique_id -or $v.FileSystemLabel -cne 'VLIKEON_QA052' -or $v.FileSystem -cne 'NTFS' -or $v.Size -gt 268435456){throw 'Volume mismatch'}; $v | Select-Object UniqueId,FileSystem,FileSystemLabel,Size,SizeRemaining | ConvertTo-Json -Compress`;
  const result=spawnSync('C:/Windows/System32/WindowsPowerShell/v1.0/powershell.exe',['-NoProfile','-Command',command],{encoding:'utf8',windowsHide:true,timeout:30000});assert.equal(result.status,0,result.stderr);return JSON.parse(result.stdout.replace(/^\uFEFF/,''));
}
function request(value){capacity();const name=`physical-request-${++serial}.json`;fs.writeFileSync(path.join(mount,name),JSON.stringify(value),{flag:'wx'});return name;}
function run(name){const result=spawnSync(exe,[mount,name],{encoding:'utf8',windowsHide:true,timeout:30000,maxBuffer:8*1024*1024});fs.writeFileSync(path.join(output,`physical-result-${name}.log`),(result.stdout??'')+'\n'+(result.stderr??''));assert.equal(result.status,0,result.stderr);return JSON.parse(result.stdout);}
function healthy(value){assert.equal(value.integrity,'ok');assert.equal(value.foreign_key_errors,false);assert.equal(value.schema,15);assert.ok(value.saves.every(save=>save.valid));}
function original(before,after){healthy(after);assert.deepEqual(after.saves,before.saves);assert.deepEqual(after.heads,before.heads);assert.equal(after.device_id,before.device_id);}
function mark(id){report.cases.push({id,status:'passed'});fs.writeFileSync(path.join(output,'physical-fault-progress.json'),JSON.stringify(report,null,2));}
function freeOwned(file){capacity();const stat=fs.lstatSync(file);assert.ok(stat.isFile()&&!stat.isSymbolicLink());assert.equal(path.dirname(file),mount);fs.unlinkSync(file);}
async function main(){
  assert.equal(path.resolve(root),expected);const receipt=JSON.parse(fs.readFileSync(path.join(root,'volume.json'),'utf8').replace(/^\uFEFF/,''));
  assert.equal(receipt.image,path.join(root,'vlikeon-storage-052.vhdx'));assert.equal(receipt.mount,path.join(root,'volume'));assert.equal(receipt.maximum_bytes,Number(maximum));assert.ok(!receipt.detached_at);
  mount=receipt.mount;report.volume=identity();report.before=capacity();
  const host=fs.statfsSync(path.dirname(root),{bigint:true});assert.ok(host.bavail*host.bsize>maximum*2n,'Insufficient host reserve for bounded VHDX growth');
  const profile='actual-full-052';assert.ok(!fs.existsSync(path.join(mount,profile)),'Fresh profile required');
  const initial=run(request({operation:'init',profile}));healthy(initial.snapshot);
  const id=crypto.randomUUID(),params={operation:'write',profile,phase:'disk_full',save_id:id,parent_id:initial.old.id,game_id:initial.game.id};
  // Create all control requests before exhaustion. Evidence remains outside VHDX.
  const failedRequest=request(params),snapshotRequest=request({operation:'snapshot',profile}),retryRequest=request(params);
  filler=path.join(mount,'physical-owned-filler.bin');reserve=path.join(mount,'physical-owned-reserve.bin');
  capacity();fs.writeFileSync(reserve,crypto.randomBytes(1024**2),{flag:'wx'});reserveCreated=true;
  capacity();const fd=fs.openSync(filler,'wx');fillerCreated=true;const chunk=crypto.randomBytes(1024**2);let written=0,failure=null;
  try{
    // 320 MiB is a hard upper bound even if filesystem accounting changes.
    for(let iteration=0;iteration<320;iteration++){
      capacity();try{const count=fs.writeSync(fd,chunk);written+=count;if(count!==chunk.length)continue;}catch(error){failure={code:error.code,errno:error.errno,syscall:error.syscall,message:error.message};break;}
    }
    try{fs.fsyncSync(fd);}catch(error){failure??={code:error.code,errno:error.errno,syscall:error.syscall,message:error.message};}
  }finally{fs.closeSync(fd);}
  report.exhaustion={written_bytes:written,error:failure,capacity:capacity()};assert.equal(failure?.code,'ENOSPC','Must observe actual filesystem exhaustion');mark('private-ntfs-write-returns-enospc');
  // Release only 1 MiB for DB bookkeeping; the actual 8-MiB save still cannot fit.
  freeOwned(reserve);reserveCreated=false;report.before_save=capacity();assert.ok(report.before_save.available_bytes<8*1024**2);
  const failed=run(failedRequest);assert.equal(failed.ok,false);assert.equal(failed.error,'storage_failure');original(initial.snapshot,failed.snapshot);mark('physical-full-save-fails-without-publishing-progress');
  freeOwned(filler);fillerCreated=false;report.after_release=capacity();
  const reopened=run(snapshotRequest);original(initial.snapshot,reopened);assert.equal(reopened.staged_or_retired,0);mark('reopen-validates-original-data-and-cleans-staging');
  const retry=run(retryRequest);assert.equal(retry.ok,true);healthy(retry.snapshot);assert.equal(retry.snapshot.saves.length,2);assert.equal(retry.snapshot.heads[0].id,id);assert.equal(retry.snapshot.staged_or_retired,0);mark('same-save-id-retries-after-space-is-released');
  report.volume_after=identity();assert.equal(report.volume_after.UniqueId,report.volume.UniqueId);report.status='passed';
}
main().catch(error=>{report.status='failed';report.error=error.stack;console.error(error);process.exitCode=1;}).finally(()=>{
  // Preserve profile/image/evidence. Reclaim only files this run created.
  try{if(reserveCreated)freeOwned(reserve);if(fillerCreated)freeOwned(filler);}catch(error){report.cleanup_error=error.stack;report.status='failed';process.exitCode=1;}
  report.finished_at=new Date().toISOString();fs.writeFileSync(path.join(output,'physical-fault-result.json'),JSON.stringify(report,null,2));
});
