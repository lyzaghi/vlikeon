const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict'),{spawnSync}=require('node:child_process');
const base=__dirname,source=base+'/source-054a',output=base+'/runs/054a';
const python='C:/Users/lyzag/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe',msvc='E:/V10/tools/VS2026BuildTools/VC/Tools/MSVC/14.51.36231',kits='C:/Program Files (x86)/Windows Kits/10',sdk='10.0.28000.0';
const hash=file=>crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'),manifest=JSON.parse(fs.readFileSync(base+'/source-054a.json','utf8'));
const unchanged=()=>Object.entries(manifest.sha256).every(([name,sha])=>hash(path.join(source,name))===sha);
assert.equal(process.platform,'win32');assert.ok(unchanged());assert.ok(!fs.existsSync(output));fs.mkdirSync(output,{recursive:true});
const env={...process.env,PYTHONDONTWRITEBYTECODE:'1',PATH:msvc+'/bin/Hostx64/x64;'+process.env.PATH,LIB:msvc+'/lib/x64;'+kits+'/Lib/'+sdk+'/ucrt/x64;'+kits+'/Lib/'+sdk+'/um/x64',INCLUDE:msvc+'/include;'+kits+'/Include/'+sdk+'/ucrt;'+kits+'/Include/'+sdk+'/shared;'+kits+'/Include/'+sdk+'/um'};
const report={item:54,batch:'054a',scope:'Portable bounded media ownership and build recipe refusal only; no macOS SDK/ScreenCaptureKit execution',executor:'Antigravity',cases:[],source_manifest_sha256:hash(base+'/source-054a.json'),started_at:new Date().toISOString()};
function run(id,command,args,expected=0,timeout=60000){const r=spawnSync(command,args,{cwd:output,env,encoding:'utf8',windowsHide:true,timeout,maxBuffer:1024**2});fs.writeFileSync(output+'/'+id+'.stdout.log',r.stdout??'');fs.writeFileSync(output+'/'+id+'.stderr.log',r.stderr??'');assert.equal(r.error,undefined,id);assert.equal(r.status,expected,id+': '+r.stderr);report.cases.push({id,status:'passed',exit_code:r.status});}
try{
 run('build-real-mailbox-fixture',msvc+'/bin/Hostx64/x64/cl.exe',['/nologo','/std:c++20','/EHsc','/W4','/WX','/O2','/DNDEBUG','/I'+source+'/engines/capture-macos/include',source+'/qa/macos/capture-mailbox.cpp','/Fe:'+output+'/capture-mailbox.exe','/Fo:'+output+'/capture-mailbox.obj']);
 run('bounded-media-ownership-and-concurrent-close',output+'/capture-mailbox.exe',[]);
 run('parse-mac-build-recipe',python,['-B','-c',"import ast,pathlib; ast.parse(pathlib.Path("+JSON.stringify(source+'/tools/build_macos_capture.py')+").read_text(encoding='utf-8'))"]);
 run('refuse-mac-build-on-windows',python,['-B',source+'/tools/build_macos_capture.py','--cmake',process.execPath,'--architecture','arm64'],2);
 report.status='passed';
}catch(error){report.status='failed';report.error=error.stack;process.exitCode=1;}
report.source_unchanged=unchanged();if(!report.source_unchanged){report.status='failed';process.exitCode=1;}report.finished_at=new Date().toISOString();fs.writeFileSync(output+'/result.json',JSON.stringify(report,null,2));console.log(JSON.stringify(report));
