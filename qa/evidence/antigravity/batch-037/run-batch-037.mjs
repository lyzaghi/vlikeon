import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
const root=path.dirname(fileURLToPath(import.meta.url));
const output=path.join(root,'runs','batch-037');
const gh='E:/V10/tools/github-cli/2.103.0/bin/gh.exe';
const sha='b1f7439ba1cabd9d025e701c9fbf99cade3b3d5b';
const title=`Linux qualification vlikeon-037 ${sha}`;
const workflow='linux-native-qualification.yml';
if(fs.existsSync(output))throw Error('Existing evidence: do not replay');
fs.mkdirSync(output,{recursive:true});
const report={batch_id:'vlikeon-037',source_commit:sha,executor:'antigravity',started_at:new Date().toISOString(),commands:[]};
function call(name,args,timeout=60000){
 const start=Date.now();const result=spawnSync(gh,args,{cwd:root,encoding:'utf8',shell:false,windowsHide:true,timeout,maxBuffer:32*1024*1024});
 fs.writeFileSync(path.join(output,name+'.stdout.log'),result.stdout||'');fs.writeFileSync(path.join(output,name+'.stderr.log'),result.stderr||'');
 report.commands.push({id:name,args,exit_code:result.status,elapsed_ms:Date.now()-start,error:result.error?{code:result.error.code,message:result.error.message}:null});
 return result;
}
function requireOkay(result){if(result.error||result.status!==0)throw Error('Assigned command failed; inspect retained raw logs');return result.stdout;}
const listArgs=['run','list','--repo','lyzaghi/vlikeon','--workflow',workflow,'--event','workflow_dispatch','--branch','main','--limit','30','--json','databaseId,headSha,displayTitle,status,conclusion,url'];
try{
 const previous=JSON.parse(requireOkay(call('prior-runs',listArgs)));
 if(previous.some(run=>run.displayTitle===title))throw Error('Existing assigned remote run: no duplicate dispatch');
 requireOkay(call('dispatch',['workflow','run',workflow,'--repo','lyzaghi/vlikeon','--ref','main','-f',`expected_sha=${sha}`,'-f','batch_id=vlikeon-037']));
 let run;
 for(let index=0;index<6;index++){
  const runs=JSON.parse(requireOkay(call('locate-'+index,listArgs)));const candidates=runs.filter(item=>item.headSha===sha&&item.displayTitle===title);
  if(candidates.length>1)throw Error('Ambiguous remote run');
  if(candidates.length===1){run=candidates[0];break;}
  await new Promise(resolve=>setTimeout(resolve,10000));
 }
 if(!run)throw Error('Dispatched run not located; reconcile before any retry');
 report.run=run;fs.writeFileSync(path.join(output,'in-progress.json'),JSON.stringify(report,null,2));
 const watched=call('watch',['run','watch',String(run.databaseId),'--repo','lyzaghi/vlikeon','--exit-status','--interval','30'],25*60*1000);
 report.observed=JSON.parse(requireOkay(call('remote-result',['run','view',String(run.databaseId),'--repo','lyzaghi/vlikeon','--json','headSha,status,conclusion,jobs,url'])));
 requireOkay(call('remote-logs',['run','view',String(run.databaseId),'--repo','lyzaghi/vlikeon','--log'],120000));
 report.status=!watched.error&&watched.status===0&&report.observed.headSha===sha&&report.observed.status==='completed'&&report.observed.conclusion==='success'?'completed':'failed';
}catch(error){report.status='blocked';report.error=String(error.message||error);}
report.finished_at=new Date().toISOString();fs.writeFileSync(path.join(output,'result.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report));process.exitCode=report.status==='completed'?0:1;
