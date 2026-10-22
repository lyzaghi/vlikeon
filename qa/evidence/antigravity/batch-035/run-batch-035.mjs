// Fixed native-worker/fixture build batch. No product execution or edits.
import fs from 'node:fs';import path from 'node:path';
import {fileURLToPath} from 'node:url';import {createHash} from 'node:crypto';import {spawnSync} from 'node:child_process';
const root=path.dirname(fileURLToPath(import.meta.url)),source=path.join(root,'source-04cbd74'),build=path.join(root,'build-batch-035'),evidence=path.join(root,'runs','batch-035');
const sdk=path.join(root,'native-source-v51');
const cmake='E:/V10/OmniPlay/out/toolchains/cmake-3.31.6-windows-v1/toolchain/bin/cmake.exe';
function inventory(){const values={};let total=0;function visit(relative='',depth=0){if(depth>32)throw new Error('Source depth');for(const item of fs.readdirSync(path.join(source,relative),{withFileTypes:true})){const name=path.posix.join(relative,item.name);if(item.isSymbolicLink())throw new Error('Source link');if(item.isDirectory())visit(name,depth+1);else if(item.isFile()){const bytes=fs.readFileSync(path.join(source,name));total+=bytes.length;if(total>128*1024*1024||Object.keys(values).length>=5000)throw new Error('Source budget');values[name]=createHash('sha256').update(bytes).digest('hex');}else throw new Error('Unknown source entry');}}visit();return values;}
const dependencyManifest=JSON.parse(fs.readFileSync(path.join(root,'batch-031-source.json'),'utf8')).sha256;
const dependencyHashes=Object.fromEntries(Object.entries(dependencyManifest).filter(([name])=>['SDL/','json/','interface/'].some(prefix=>name.startsWith(prefix))));
const dependenciesOkay=()=>Object.entries(dependencyHashes).every(([name,digest])=>createHash('sha256').update(fs.readFileSync(path.join(sdk,name))).digest('hex')===digest);
if(!dependenciesOkay())throw Error('Pinned dependency mismatch');
const before=inventory(),expected=JSON.parse(fs.readFileSync(path.join(root,'batch-035-source.json'),'utf8'));
if(JSON.stringify(Object.entries(before).sort())!==JSON.stringify(Object.entries(expected.sha256).sort()))throw new Error('Source mismatch');
if(!fs.existsSync(cmake)||fs.existsSync(build)||fs.existsSync(evidence))throw new Error('Prerequisite or existing evidence; do not replay');
fs.mkdirSync(build);fs.mkdirSync(evidence,{recursive:true});
const worker=path.join(build,'worker'),fixture=path.join(build,'fixtures'),peer=path.join(build,'peer');
const steps=[
 {id:'worker-configure',args:['-S',path.join(source,'engines/native'),'-B',worker,'-G','Visual Studio 17 2022','-A','x64',`-DOMNIPLAY_SDL_SOURCE=${path.join(sdk,'SDL')}`,`-DOMNIPLAY_JSON_SOURCE=${path.join(sdk,'json')}`,`-DOMNIPLAY_LIBRETRO_INCLUDE=${path.join(sdk,'interface','libretro')}`,`-DOMNIPLAY_PRESENTATION_SOURCE=${path.join(source,'engines/presentation')}`,'-DOMNIPLAY_RUNTIME_ID=native-lifecycle-fixture','-DOMNIPLAY_CORE_FILENAME=omniplay-lifecycle-fixture.dll','-DOMNIPLAY_VIDEO_BACKEND=software','-DOMNIPLAY_BUILD_PRESENTATION_HOST=OFF','-DOMNIPLAY_DEVICE_FREE_ACTOR=OFF','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded','-DSDL_REVISION=SDL-3.4.18-829a65d769d935c4852f8159e964312c0957260a']},
 {id:'worker-build',args:['--build',worker,'--config','Release','--target','omniplay-native-worker','--parallel','2']},

 {id:'core-configure',args:['-S',path.join(source,'qa/native-lifecycle'),'-B',fixture,'-G','Visual Studio 17 2022','-A','x64','-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded',`-DOMNIPLAY_LIBRETRO_INCLUDE=${path.join(sdk,'interface/libretro')}`]},
 {id:'core-build',args:['--build',fixture,'--config','Release','--parallel','2']},

];
const report={batch_id:'vlikeon-035',source_commit:expected.source_commit,executor:'antigravity',started_at:new Date().toISOString(),cases:[],before_sha256:before,artifacts:{}};
for(const step of steps){const started=Date.now(),result=spawnSync(cmake,step.args,{cwd:build,encoding:'utf8',shell:false,windowsHide:true,timeout:step.id==='worker-build'?480000:120000,maxBuffer:4*1024*1024});fs.writeFileSync(path.join(evidence,step.id+'.stdout.log'),result.stdout||'');fs.writeFileSync(path.join(evidence,step.id+'.stderr.log'),result.stderr||'');report.cases.push({id:step.id,args:step.args,elapsed_ms:Date.now()-started,exit_code:result.status,signal:result.signal,error:result.error?{code:result.error.code,message:result.error.message}:null,status:result.error?'blocked':result.status===0?'completed':'failed'});if(result.error||result.status!==0)break;}
for(const relative of ['worker/Release/omniplay-native-worker.exe','fixtures/Release/omniplay-lifecycle-fixture.dll','fixtures/Release/omniplay-lifecycle-void-fixture.dll']){const file=path.join(build,relative);if(fs.existsSync(file))report.artifacts[relative]={bytes:fs.statSync(file).size,sha256:createHash('sha256').update(fs.readFileSync(file)).digest('hex')};}


if(report.cases.length===4&&report.cases.every(c=>c.exit_code===0&&!c.error)){
 const python='C:/Users/lyzag/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe';
 const workerFile=path.join(worker,'Release/omniplay-native-worker.exe'),core=path.join(fixture,'Release/omniplay-lifecycle-fixture.dll'),voidCore=path.join(fixture,'Release/omniplay-lifecycle-void-fixture.dll');
 for(const name of ['temporary','lifecycle','wire']){
  const args=['-B','-X','utf8',path.join(source,'qa/native-lifecycle',{temporary:'temporary_directory_cases.py',lifecycle:'run_cases.py',wire:'wire_cases.py'}[name]),'--worker',workerFile,'--core',core];
  if(name==='lifecycle')args.push('--void-core',voidCore);
  args.push('--output',path.join(evidence,name));
  const result=spawnSync(python,args,{cwd:source,encoding:'utf8',shell:false,windowsHide:true,timeout:180000,maxBuffer:1024*1024});
  fs.writeFileSync(path.join(evidence,name+'.stdout.log'),result.stdout||'');fs.writeFileSync(path.join(evidence,name+'.stderr.log'),result.stderr||'');
  report.cases.push({id:name,args,exit_code:result.status,error:result.error?{code:result.error.code,message:result.error.message}:null,status:result.error?'blocked':result.status===0?'completed':'failed'});
  if(result.error||result.status!==0)break;
 }
}
report.dependencies_unchanged=dependenciesOkay();report.after_sha256=inventory();report.source_unchanged=JSON.stringify(Object.entries(before).sort())===JSON.stringify(Object.entries(report.after_sha256).sort());report.finished_at=new Date().toISOString();report.status=!report.source_unchanged||!report.dependencies_unchanged?'source_changed':report.cases.some(item=>item.status==='failed')?'failed':report.cases.some(item=>item.status==='blocked')?'blocked':report.cases.length===7?'completed':'incomplete';
fs.writeFileSync(path.join(evidence,'result.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({batch_id:report.batch_id,status:report.status,evidence,cases:report.cases,artifacts:report.artifacts,source_unchanged:report.source_unchanged}));process.exitCode=report.status==='completed'?0:1;
