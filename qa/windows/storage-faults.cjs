// Native failure runner; terminate only child handles created by this runner.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto'),{spawn,spawnSync}=require('node:child_process');
const output=process.env.VLIKEON_QA_OUTPUT,exe=process.env.VLIKEON_STORAGE_EXE,root=path.join(output,'native-faults');fs.mkdirSync(root);let serial=0;
const report={item:52,scope:'native crash/abort/SQLite capacity/migrations',status:'running',cases:[]};
function request(value){const name=`request-${++serial}.json`;fs.writeFileSync(path.join(root,name),JSON.stringify(value));return name;}
function run(value){const name=request(value),result=spawnSync(exe,[root,name],{encoding:'utf8',windowsHide:true,timeout:30000,maxBuffer:8*1024*1024});fs.writeFileSync(path.join(root,`result-${serial}.log`),(result.stdout??'')+'\n'+(result.stderr??''));assert.equal(result.status,0,result.stderr);return JSON.parse(result.stdout);}
function healthy(snapshot){assert.equal(snapshot.integrity,'ok');assert.equal(snapshot.foreign_key_errors,false);assert.equal(snapshot.schema,15);assert.ok(snapshot.saves.every(save=>save.valid));}
function sameProgress(before,after){assert.deepEqual(after.saves,before.saves);assert.deepEqual(after.heads,before.heads);healthy(after);}
async function mark(id,body){await body();report.cases.push({id,status:'passed'});fs.writeFileSync(path.join(output,'native-fault-progress.json'),JSON.stringify(report,null,2));}
async function killAt(phase){
  const profile=`crash-${phase}`,initial=run({operation:'init',profile}),id=crypto.randomUUID(),marker=`marker-${phase}`;
  const name=request({operation:'write',profile,phase,save_id:id,parent_id:initial.old.id,game_id:initial.game.id,marker});
  const child=spawn(exe,[root,name],{windowsHide:true,stdio:['ignore','pipe','pipe']});let stderr='';child.stderr.on('data',data=>stderr+=data);child.stdout.on('data',()=>{});
  const ended=new Promise(resolve=>child.once('exit',(code,signal)=>resolve({code,signal})));let exit=null;void ended.then(value=>exit=value);
  try{
    const deadline=Date.now()+30000;
    while(!fs.existsSync(path.join(root,marker))||fs.readFileSync(path.join(root,marker),'utf8')!==phase){if(exit)throw Error('Child exited before phase '+phase+': '+stderr);if(Date.now()>deadline)throw Error('Phase marker timeout '+phase);await new Promise(resolve=>setTimeout(resolve,50));}
    assert.equal(fs.readFileSync(path.join(root,marker),'utf8'),phase);assert.ok(!exit,'Child was not alive at phase marker');
    assert.equal(child.kill('SIGKILL'),true);const terminal=await ended;assert.ok(terminal.code!==0||terminal.signal);
    const after=run({operation:'snapshot',profile});healthy(after);assert.equal(after.staged_or_retired,0);
    if(phase==='committed'){assert.equal(after.saves.length,2);assert.equal(after.heads[0].id,id);}else{
      sameProgress(initial.snapshot,after);const retried=run({operation:'write',profile,phase:'normal',save_id:id,parent_id:initial.old.id,game_id:initial.game.id});assert.equal(retried.ok,true);assert.equal(retried.snapshot.heads[0].id,id);
    }
  }finally{if(!exit){child.kill('SIGKILL');await ended;}}
}
async function main(){
  for(let version=0;version<=15;version++)await mark(`schema-${version}-to-15-preserves-data-and-reopen`,()=>{const result=run({operation:'migration',profile:'schema-'+version,version});healthy(result);});
  for(let version=10;version<=13;version++)await mark(`migration-${version}-sqlite-full-rollback-and-retry`,()=>{const result=run({operation:'migration',profile:'migration-full-'+version,version,interrupt:true});healthy(result);assert.equal(result.migration_rollback_verified,true);});
  for(const phase of ['abort','sqlite_full'])await mark(`publication-${phase}-rollback-cleanup-retry`,()=>{
    const profile='failure-'+phase,initial=run({operation:'init',profile}),id=crypto.randomUUID(),params={operation:'write',profile,save_id:id,parent_id:initial.old.id,game_id:initial.game.id};
    const failed=run({...params,phase});assert.equal(failed.ok,false);sameProgress(initial.snapshot,failed.snapshot);
    const reopened=run({operation:'snapshot',profile});sameProgress(initial.snapshot,reopened);assert.equal(reopened.staged_or_retired,0);
    const retried=run({...params,phase:'normal'});assert.equal(retried.ok,true);healthy(retried.snapshot);assert.equal(retried.snapshot.heads[0].id,id);
  });
  for(const phase of ['reservation','payload_synced','metadata_inserted','pointer_published','head_updated','committed'])await mark('process-crash-'+phase,()=>killAt(phase));
  report.status='passed';
}
main().catch(error=>{report.status='failed';report.error=error.stack;console.error(error);process.exitCode=1;}).finally(()=>fs.writeFileSync(path.join(output,'native-fault-result.json'),JSON.stringify(report,null,2)));
