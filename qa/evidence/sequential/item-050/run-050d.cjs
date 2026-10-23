const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),{spawnSync}=require('node:child_process');
const base=__dirname,source=base+'/source-050d',output=base+'/runs/050d';
const python='C:/Users/lyzag/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe',rust='E:/V10/OmniPlay/out/toolchains/rust-1.89.0-windows-msvc-v1/toolchain/bin';
const msvc='E:/V10/tools/VS2026BuildTools/VC/Tools/MSVC/14.51.36231',kits='C:/Program Files (x86)/Windows Kits/10',sdk='10.0.28000.0';
const hash=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'),manifest=JSON.parse(fs.readFileSync(base+'/source-050d.json','utf8'));
const unchanged=()=>Object.entries(manifest.sha256).every(([name,sha])=>hash(path.join(source,name))===sha);
if(fs.existsSync(output)||!unchanged())throw Error('Fresh output and immutable source required');fs.mkdirSync(output,{recursive:true});
const env={...process.env,PYTHONDONTWRITEBYTECODE:'1',CARGO_HOME:'E:/V10/work/vlikeon-qa-20261023/cargo-home',CARGO_TARGET_DIR:base+'/target-050',RUSTC:rust+'/rustc.exe',RUSTDOC:rust+'/rustdoc.exe',CARGO_TERM_COLOR:'never',CARGO_BUILD_JOBS:'2',PATH:rust+';'+msvc+'/bin/Hostx64/x64;'+process.env.PATH,LIB:msvc+'/lib/x64;'+kits+'/Lib/'+sdk+'/ucrt/x64;'+kits+'/Lib/'+sdk+'/um/x64',INCLUDE:msvc+'/include;'+kits+'/Include/'+sdk+'/ucrt;'+kits+'/Include/'+sdk+'/shared;'+kits+'/Include/'+sdk+'/um',VLIKEON_QA_OUTPUT:output,VLIKEON_WEB_BUNDLE:base+'/generated-050d/out/web',VLIKEON_CHROME:'C:/Program Files/Google/Chrome/Application/chrome.exe',VLIKEON_PLAYWRIGHT_MODULE:'C:/Users/lyzag/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright'};
delete env.OMNIPLAY_UPDATE_CHANNEL_JSON;
const report={item:50,batch:'050d',executor:'antigravity',source_manifest_sha256:hash(base+'/source-050d.json'),started_at:new Date().toISOString(),runner_pid:process.pid,cases:[]};
fs.writeFileSync(output+'/running.json',JSON.stringify(report,null,2));
function run(id,command,args,timeout=120000){
 const started=Date.now(),result=spawnSync(command,args,{cwd:source,env,encoding:'utf8',windowsHide:true,timeout,maxBuffer:8*1024*1024});
 fs.writeFileSync(output+'/'+id+'.stdout.log',result.stdout??'');fs.writeFileSync(output+'/'+id+'.stderr.log',result.stderr??'');
 report.cases.push({id,args,exit_code:result.status,error:result.error?.message,elapsed_ms:Date.now()-started,status:!result.error&&result.status===0?'passed':'failed'});fs.writeFileSync(output+'/progress.json',JSON.stringify(report,null,2));
}
run('native-recovery',rust+'/cargo.exe',['test','-p','omniplay-core','--test','manual_recovery','--locked','--offline','--','--test-threads=1'],360000);
run('native-consumers',rust+'/cargo.exe',['check','-p','omniplay-broker','-p','omniplay-local-service','-p','omniplay-mobile','--all-targets','--locked','--offline'],360000);
run('javascript',process.execPath,['--experimental-vm-modules','--test','qa/browser/manual-recovery.test.mjs','qa/browser/component-ownership.test.mjs']);
run('source-generation',python,['-B','qa/integration/rebuild_resources.py','--source',source,'--work',base+'/generated-050d','--python',python],300000);
run('browser-integration',process.execPath,['qa/browser/manual-recovery-ui.cjs']);
report.source_unchanged=unchanged();report.status=report.source_unchanged&&report.cases.length===5&&report.cases.every(value=>value.status==='passed')?'passed':'failed';report.finished_at=new Date().toISOString();
fs.writeFileSync(output+'/result.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report));process.exitCode=report.status==='passed'?0:1;
