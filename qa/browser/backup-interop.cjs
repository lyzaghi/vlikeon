// Execute only from an assigned, frozen QA batch. All content is synthetic.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {spawnSync}=require('node:child_process'),{createHash}=require('node:crypto');
const {chromium}=require(process.env.VLIKEON_PLAYWRIGHT_MODULE);
const output=process.env.VLIKEON_QA_OUTPUT,root=path.resolve(process.env.VLIKEON_WEB_BUNDLE),exe=process.env.VLIKEON_INTEROP_EXE;
const work=path.join(output,'exchange');fs.mkdirSync(work);let serial=0;
const sha=value=>createHash('sha256').update(value).digest('hex');
function native(operation,profile,params={}){
  const name=`request-${++serial}.json`;fs.writeFileSync(path.join(work,name),JSON.stringify({operation,profile,...params}));
  const result=spawnSync(exe,[work,name],{encoding:'utf8',windowsHide:true,timeout:120000,maxBuffer:8*1024*1024});
  fs.writeFileSync(path.join(work,`response-${serial}.log`),(result.stdout??'')+'\n'+(result.stderr??''));
  assert.equal(result.status,0,`native ${operation}: ${result.stderr}`);return JSON.parse(result.stdout);
}
function archive(name){const data=fs.readFileSync(path.join(work,name)),length=data.readUInt32LE(8);return{data,manifest:JSON.parse(data.subarray(12,12+length).toString('utf8')),payload:data.subarray(12+length,-32)};}
function variant(name,source,modify){const {manifest,payload}=archive(source);const result=modify(manifest,payload);const json=Buffer.from(JSON.stringify(manifest)),header=Buffer.alloc(12);header.write('OPBK0001');header.writeUInt32LE(json.length,8);const body=Buffer.concat([header,json,result??payload]),trailer=Buffer.from(sha(Buffer.concat([Buffer.from('Vlikeon portable saves archive v1\0'),body])),'hex');fs.writeFileSync(path.join(work,name),Buffer.concat([body,trailer]));}
const heads=snapshot=>snapshot.heads.map(value=>value.id??value.save_id).sort();
function assertImported(result,source,before){
  assert.equal(result.result.ok,true);assert.ok(result.result.value.entries.every(item=>item.error===null));assert.deepEqual(heads(result.after),heads(before));
  for(const entry of result.result.value.entries){
    const record=result.after.records.find(record=>record.metadata.id===entry.restored_id),original=source.entries.find(value=>value.metadata.id===entry.source_id);
    assert.ok(record,entry.restored_id);assert.equal(record.origin.kind,'imported');assert.equal(record.metadata.conflict,true);assert.equal(record.metadata.parent_id,null);
    assert.equal(record.payload.valid,true);assert.equal(record.metadata.payload_hash,original.metadata.payload_hash);assert.deepEqual(record.reference,original);
    assert.deepEqual(record.origin.imported_from,original.origin.canonical_metadata??original.metadata);
  }
}
const server=http.createServer((req,res)=>{
  if(req.url==='/fixture.html'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><title>Archive interoperability fixture</title>');return;}
  if(/^\/qa-archive\/[a-z0-9-]+\.opbackup$/.test(req.url)){const file=path.join(work,req.url.slice('/qa-archive/'.length));if(!fs.existsSync(file)){res.writeHead(404);res.end();return;}res.setHeader('Content-Type','application/octet-stream');fs.createReadStream(file).pipe(res);return;}
  const file=path.resolve(root,'.'+decodeURIComponent(req.url.split('?')[0]));
  if(!file.startsWith(root+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404);res.end();return;}
  res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':'application/octet-stream');fs.createReadStream(file).pipe(res);
});
const report={item:51,status:'running',cases:[],artifacts:{}};
async function main(){
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));let browser;
  try{
    browser=await chromium.launch({executablePath:process.env.VLIKEON_CHROME,headless:true});report.browser=await browser.version();
    async function page(games,template,seed=false){
      const page=await browser.newPage();await page.goto(`http://127.0.0.1:${server.address().port}/fixture.html`);
      await page.evaluate(async({games,template,seed})=>{
        const {openStore}=await import('/web/idb.js'),{WebSaves}=await import('/web/saves.js'),{WebSaveBackup}=await import('/web/save-backup.js'),{Sha256}=await import('/runtime/sha256.js');
        const store=await openStore(),saves=new WebSaves(store),backup=new WebSaveBackup(store,saves),local=games.map(game=>({...game,id:crypto.randomUUID()}));
        await store.transaction(['games','config'],'readwrite',async tx=>{for(const game of local)await tx.add('games',{...game,sort_title:game.title.toLowerCase(),favorite:0});const usage=await tx.get('config','usage');await tx.put('config',{key:'usage',value:{...usage.value,game_count:local.length}});});
        const payload=text=>new Blob([text]);
        async function commit(game,text,slot,options={}){
          const blob=payload(text),hash=new Sha256().update(new TextEncoder().encode(text)).hex(),id=crypto.randomUUID();
          const metadata={...template,id,game_id:game.id,game_content_hash:game.content_hash,kind:options.state?'emulator_state':'in_game',slot,parent_id:null,generation:1,label:options.label??null,conflict:Boolean(options.imported),payload_hash:hash,payload_bytes:blob.size};
          const origin=options.remote?{...metadata,game_id:crypto.randomUUID(),parent_id:crypto.randomUUID(),generation:777}:options.imported?{...metadata,id:crypto.randomUUID()}:null;
          const saved=await saves.commit(metadata,blob,null,options.remote?{remoteMetadata:origin}:options.imported?{importedFrom:origin}:{});
          if(options.legacy)await store.transaction(['save_origins'],'readwrite',tx=>tx.put('save_origins',{schema_version:1,save_id:id,kind:'legacy_projection',canonical_metadata:null,canonical_hash:null,imported_from:null,imported_from_hash:null,missing_parent:false}));
          return saved;
        }
        for(const game of local)await commit(game,'Existing Web head',0,{label:'Giữ nguyên head 🎮'});
        if(seed){await commit(local[0],'web captured',1,{label:'Tiếng Việt 日本語 🎮'});await commit(local[0],'web legacy',2,{legacy:true,label:''});await commit(local[0],'web remote',3,{remote:true});await commit(local[0],'web imported',4,{imported:true});await commit(local[0],'web state',5,{state:true,label:'\" \\ \n 🚀'});}
        window.api={store,saves,backup,games:local,async snapshot(){
          const captured=await store.transaction(['saves','heads','save_origins','config','removed_saves'],'readonly',async tx=>({raw:await tx.all('saves'),heads:await tx.all('heads'),origins:await tx.all('save_origins'),references:await tx.all('config'),removed:await tx.all('removed_saves')}));
          const records=[];for(const {payload,...metadata}of captured.raw){let bytes;try{const value=await saves.read(metadata.id);bytes={valid:true,base64:value.payload.size<=4096?btoa(String.fromCharCode(...new Uint8Array(await value.payload.arrayBuffer()))):null};}catch(error){bytes={valid:false,error:error.code};}const reference=captured.references.find(value=>value.key===`backup_reference.${metadata.id}`)?.value;records.push({metadata,origin:captured.origins.find(value=>value.save_id===metadata.id),reference:reference===undefined?null:JSON.parse(reference),payload:bytes});}
          return{records:records.sort((a,b)=>a.metadata.id.localeCompare(b.metadata.id)),heads:captured.heads,removed:captured.removed.map(value=>value.id).sort()};
        }};
      },{games,template,seed});return page;
    }
    const seed=native('init','native-source',{seed:true}),template=seed.snapshot.records[0].metadata;
    const ids=seed.snapshot.records.map(value=>value.metadata.id),exported=native('export','native-source',{ids,archive:'native.opbackup'});assert.equal(exported.result.ok,true);
    const source=archive('native.opbackup');assert.equal(source.manifest.entries.length,7);
    const origins=new Set(source.manifest.entries.map(value=>value.origin.kind));for(const kind of ['captured','legacy_projection','authenticated_remote','imported'])assert.ok(origins.has(kind));
    assert.ok(source.manifest.entries.some(value=>value.origin.missing_parent));assert.ok(source.manifest.entries.some(value=>value.origin.canonical_metadata&&value.origin.canonical_metadata.generation!==value.metadata.generation));
    const p=await page(seed.games,template),q=await page(seed.games,template,true);
    async function webRestore(page,name){const data=fs.readFileSync(path.join(work,name));return page.evaluate(async base64=>{const before=await api.snapshot();try{const value=await api.backup.restore(new Blob([Uint8Array.from(atob(base64),c=>c.charCodeAt(0))]));return{result:{ok:true,value},before,after:await api.snapshot()};}catch(error){return{result:{ok:false,error:error.code,message:error.message},before,after:await api.snapshot()};}},data.toString('base64'));}
    async function webExport(page,name,ids){const result=await page.evaluate(async ids=>{const value=await api.backup.export(ids);return{base64:btoa(String.fromCharCode(...new Uint8Array(await value.blob.arrayBuffer()))),hash:value.archive_hash};},ids);fs.writeFileSync(path.join(work,name),Buffer.from(result.base64,'base64'));return archive(name);}
    async function mark(name,body){await body();report.cases.push({id:name,status:'passed'});fs.writeFileSync(path.join(output,'interop-progress.json'),JSON.stringify(report,null,2));}
    let webImported,nativeImported;
    await mark('native-to-web-preserves-bytes-origins-heads',async()=>{webImported=await webRestore(p,'native.opbackup');assertImported(webImported,source.manifest,webImported.before);});
    const target=native('init','native-target',{heads:true});
    await mark('same-archive-derives-identical-import-ids',async()=>{nativeImported=native('restore','native-target',{archive:'native.opbackup'});assertImported(nativeImported,source.manifest,target.snapshot);assert.deepEqual(nativeImported.result.value,webImported.result.value);});
    await mark('retry-is-idempotent-on-both-implementations',async()=>{
      const a=native('restore','native-target',{archive:'native.opbackup'}),b=await webRestore(p,'native.opbackup');
      for(const value of [a,b]){assert.ok(value.result.value.entries.every(entry=>entry.already_present));assert.deepEqual(value.before,value.after);}
    });
    await mark('web-to-native-captured-legacy-remote-imported',async()=>{
      const snapshot=await q.evaluate(()=>api.snapshot()),webArchive=await webExport(q,'web.opbackup',snapshot.records.map(value=>value.metadata.id));
      const before=native('init','web-target',{heads:true});const restored=native('restore','web-target',{archive:'web.opbackup'});assertImported(restored,webArchive.manifest,before.snapshot);
    });
    await mark('native-web-native-and-web-native-web-reexport-references',async()=>{
      const a=await webExport(p,'web-reexport.opbackup',webImported.result.value.entries.map(value=>value.restored_id));
      assert.ok(a.manifest.entries.every(value=>value.prior.length===1));
      const init=native('init','roundtrip',{heads:true}),restored=native('restore','roundtrip',{archive:'web-reexport.opbackup'});assertImported(restored,a.manifest,init.snapshot);
      const imported=native('snapshot','web-target').after.records.filter(value=>value.reference).map(value=>value.metadata.id);
      assert.equal(native('export','web-target',{ids:imported,archive:'native-reexport.opbackup'}).result.ok,true);
      const b=archive('native-reexport.opbackup'),restoredWeb=await webRestore(p,'native-reexport.opbackup');assertImported(restoredWeb,b.manifest,restoredWeb.before);
    });
    await mark('explicit-purge-reimport-keeps-suppression-and-new-identities',async()=>{
      for(const game of target.games)native('purge','native-target',{game_id:game.id});
      const a=native('restore','native-target',{archive:'native.opbackup'});assert.ok(a.result.value.entries.every(entry=>entry.error===null&&!entry.already_present));
      const oldIds=new Set(nativeImported.result.value.entries.map(entry=>entry.restored_id));assert.ok(a.result.value.entries.every(entry=>!oldIds.has(entry.restored_id)));assert.ok([...oldIds].every(id=>a.after.removed.includes(id)));
      await p.evaluate(async()=>{for(const game of api.games){const preview=await api.saves.purgePreview(game.id);await api.saves.purge(game.id,preview.revision);}});
      const b=await webRestore(p,'native.opbackup');assert.deepEqual(a.result.value,b.result.value);assert.ok([...oldIds].every(id=>b.after.removed.includes(id)));
    });
    await mark('partial-missing-and-active-games-report-per-entry',async()=>{
      const init=native('init','partial',{games:1});native('active','partial',{game_id:init.games[0].id});
      const a=native('restore','partial',{archive:'native.opbackup'});assert.ok(a.result.value.entries.some(entry=>entry.error==='busy'));assert.ok(a.result.value.entries.some(entry=>entry.error==='not_found'));assert.equal(a.after.records.length,0);
      native('stop','partial');const b=native('restore','partial',{archive:'native.opbackup'});assert.equal(b.result.value.entries.filter(entry=>entry.error===null).length,6);assert.equal(b.result.value.entries.filter(entry=>entry.error==='not_found').length,1);
      const partialWeb=await page(seed.games.slice(0,1),template),c=await webRestore(partialWeb,'native.opbackup');assert.equal(c.result.value.entries.filter(entry=>entry.error===null).length,6);assert.equal(c.result.value.entries.filter(entry=>entry.error==='not_found').length,1);await partialWeb.close();
      const activeWeb=await page(seed.games,template);
      await activeWeb.evaluate(async()=>{await api.store.transaction(['leases'],'readwrite',tx=>tx.add('leases',{id:crypto.randomUUID(),game_id:api.games[0].id,expires_at_ms:Date.now()+60000,info:{state:'paused'}}));});
      const d=await webRestore(activeWeb,'native.opbackup');assert.equal(d.result.value.entries.filter(entry=>entry.error==='busy').length,6);assert.equal(d.result.value.entries.filter(entry=>entry.error===null).length,1);await activeWeb.close();
    });
    await mark('damaged-restored-generation-is-not-overwritten-on-retry',async()=>{
      const id=nativeImported.result.value.entries[0].restored_id,reimport=native('snapshot','native-target').after.records.find(record=>record.reference&&record.reference.metadata.id===source.manifest.entries[0].metadata.id);
      native('damage','native-target',{save_id:reimport.metadata.id});const a=native('restore','native-target',{archive:'native.opbackup'});assert.ok(a.result.value.entries.some(entry=>entry.error==='corrupt_data'));
      const selected=webImported.result.value.entries[0].source_id;
      await p.evaluate(async selected=>{const snapshot=await api.snapshot(),record=snapshot.records.find(record=>record.reference?.metadata.id===selected);await api.store.transaction(['saves'],'readwrite',async tx=>{const current=await tx.get('saves',record.metadata.id);await tx.put('saves',{...current,payload:new Blob(['bad'])});});},selected);
      const b=await webRestore(p,'native.opbackup');assert.ok(b.result.value.entries.some(entry=>entry.error==='corrupt_data'));void id;
    });
    await mark('sixty-four-entry-boundary-and-overflow',async()=>{
      const base=source.manifest.entries.find(value=>value.origin.kind==='legacy_projection'),payload=Buffer.from('legacy');
      const limitWeb=await page(seed.games,template),initial=native('init','entry-limit',{heads:true});
      variant('entries-64.opbackup','native.opbackup',manifest=>{
        manifest.entries=Array.from({length:64},()=>{const entry=structuredClone(base);entry.metadata.id=crypto.randomUUID();entry.origin.save_id=entry.metadata.id;return entry;});
        return Buffer.concat(Array.from({length:64},()=>payload));
      });
      const accepted=archive('entries-64.opbackup'),a=native('restore','entry-limit',{archive:'entries-64.opbackup'}),b=await webRestore(limitWeb,'entries-64.opbackup');
      assertImported(a,accepted.manifest,initial.snapshot);assertImported(b,accepted.manifest,b.before);assert.deepEqual(a.result.value,b.result.value);
      variant('entries-65.opbackup','entries-64.opbackup',(manifest,bytes)=>{const entry=structuredClone(manifest.entries[0]);entry.metadata.id=crypto.randomUUID();entry.origin.save_id=entry.metadata.id;manifest.entries.push(entry);return Buffer.concat([bytes,payload]);});
      for(const value of [native('restore','entry-limit',{archive:'entries-65.opbackup'}),await webRestore(limitWeb,'entries-65.opbackup')]){assert.equal(value.result.ok,false);assert.deepEqual(value.before,value.after);}
      await limitWeb.close();
    });
    await mark('sixteen-prior-references-preserved-and-reexport-overflow-refused',async()=>{
      const base=structuredClone(source.manifest.entries.find(value=>value.origin.kind==='legacy_projection'));
      variant('history-16.opbackup','native.opbackup',manifest=>{const entry=structuredClone(base);const reference={metadata:base.metadata,origin:base.origin,current:base.current};entry.prior=Array.from({length:16},()=>structuredClone(reference));manifest.entries=[entry];return Buffer.from('legacy');});
      const historyWeb=await page(seed.games,template),initial=native('init','history-limit',{heads:true}),original=archive('history-16.opbackup');
      const a=native('restore','history-limit',{archive:'history-16.opbackup'}),b=await webRestore(historyWeb,'history-16.opbackup');assertImported(a,original.manifest,initial.snapshot);assertImported(b,original.manifest,b.before);
      const ids=a.result.value.entries.map(value=>value.restored_id),exported=native('export','history-limit',{ids,archive:'history-overflow.opbackup'});assert.equal(exported.result.ok,false);assert.equal(fs.statSync(path.join(work,'history-overflow.opbackup')).size,0);
      const error=await historyWeb.evaluate(async ids=>{try{await api.backup.export(ids);return null;}catch(error){return error.code;}},ids);assert.equal(error,'resource_limit');await historyWeb.close();
    });
    await mark('exact-128mib-total-and-overflow-before-publication',async()=>{
      const max=128*1024**2,entry=structuredClone(source.manifest.entries.find(value=>value.origin.kind==='legacy_projection'));
      entry.metadata.id=crypto.randomUUID();entry.origin.save_id=entry.metadata.id;entry.metadata.payload_bytes=max;
      let json=Buffer.from(JSON.stringify({schema_version:1,entries:[entry]}));entry.metadata.payload_bytes=max-44-json.length;
      const zero=Buffer.alloc(1024**2),payloadHash=createHash('sha256');let remaining=entry.metadata.payload_bytes;
      while(remaining){const count=Math.min(zero.length,remaining);payloadHash.update(zero.subarray(0,count));remaining-=count;}entry.metadata.payload_hash=payloadHash.digest('hex');
      json=Buffer.from(JSON.stringify({schema_version:1,entries:[entry]}));assert.equal(44+json.length+entry.metadata.payload_bytes,max);
      const header=Buffer.alloc(12);header.write('OPBK0001');header.writeUInt32LE(json.length,8);const hash=createHash('sha256').update('Vlikeon portable saves archive v1\0').update(header).update(json),file=fs.openSync(path.join(work,'total-boundary.opbackup'),'wx');
      try{fs.writeSync(file,header);fs.writeSync(file,json);remaining=entry.metadata.payload_bytes;while(remaining){const count=Math.min(zero.length,remaining),chunk=zero.subarray(0,count);fs.writeSync(file,chunk);hash.update(chunk);remaining-=count;}fs.writeSync(file,hash.digest());}finally{fs.closeSync(file);}
      const limitWeb=await page(seed.games,template),initial=native('init','byte-limit',{heads:true});
      const a=native('restore','byte-limit',{archive:'total-boundary.opbackup'});assertImported(a,{entries:[entry]},initial.snapshot);
      const b=await limitWeb.evaluate(async()=>{const before=await api.snapshot(),blob=await(await fetch('/qa-archive/total-boundary.opbackup')).blob();const value=await api.backup.restore(blob);return{before,after:await api.snapshot(),result:{ok:true,value}};});assertImported(b,{entries:[entry]},b.before);assert.deepEqual(a.result.value,b.result.value);
      fs.copyFileSync(path.join(work,'total-boundary.opbackup'),path.join(work,'total-overflow.opbackup'));fs.appendFileSync(path.join(work,'total-overflow.opbackup'),Buffer.from([0]));
      const failed=native('restore','byte-limit',{archive:'total-overflow.opbackup'});assert.equal(failed.result.ok,false);assert.deepEqual(failed.before,failed.after);
      const webFailed=await limitWeb.evaluate(async()=>{const before=await api.snapshot();let error=null;try{await api.backup.restore(await(await fetch('/qa-archive/total-overflow.opbackup')).blob());}catch(value){error=value.code;}return{before,after:await api.snapshot(),error};});assert.equal(webFailed.error,'resource_limit');assert.deepEqual(webFailed.before,webFailed.after);await limitWeb.close();
    });
    await mark('actual-four-mib-manifest-boundary-and-overflow',async()=>{
      for(const length of [4*1024**2,4*1024**2+1]){
        const json=Buffer.alloc(length,0x20);Buffer.from(JSON.stringify(source.manifest)).copy(json);const header=Buffer.alloc(12);header.write('OPBK0001');header.writeUInt32LE(length,8);
        const body=Buffer.concat([header,json,source.payload]),trailer=Buffer.from(sha(Buffer.concat([Buffer.from('Vlikeon portable saves archive v1\0'),body])),'hex'),name=length===4*1024**2?'manifest-boundary.opbackup':'manifest-overflow.opbackup';fs.writeFileSync(path.join(work,name),Buffer.concat([body,trailer]));
        const pageTarget=await page(seed.games,template),profile=length===4*1024**2?'manifest-limit':'manifest-overflow',initial=native('init',profile,{heads:true});
        const a=native('restore',profile,{archive:name}),b=await pageTarget.evaluate(async name=>{const before=await api.snapshot();try{const value=await api.backup.restore(await(await fetch('/qa-archive/'+name)).blob());return{result:{ok:true,value},before,after:await api.snapshot()};}catch(error){return{result:{ok:false,error:error.code},before,after:await api.snapshot()};}},name);
        if(length===4*1024**2){assertImported(a,source.manifest,initial.snapshot);assertImported(b,source.manifest,b.before);assert.deepEqual(a.result.value,b.result.value);}else{for(const value of [a,b]){assert.equal(value.result.ok,false);assert.deepEqual(value.before,value.after);}}await pageTarget.close();
      }
    });
    await mark('oversized-retained-reference-refuses-without-truncation',async()=>{
      const original=structuredClone(source.manifest.entries.find(value=>value.origin.kind==='imported'));
      const originHash=metadata=>sha(Buffer.concat([Buffer.from('OmniPlay save origin v1\0'),Buffer.from(JSON.stringify(Object.fromEntries(Object.keys(metadata).sort().map(key=>[key,metadata[key]]))))]));
      for(const metadata of [original.metadata,original.origin.canonical_metadata,original.origin.imported_from]){metadata.label='\0'.repeat(256);metadata.state_format_version='\0'.repeat(512);metadata.platform='\0'.repeat(128);}
      original.origin.canonical_hash=originHash(original.origin.canonical_metadata);original.origin.imported_from_hash=originHash(original.origin.imported_from);
      const reference={metadata:original.metadata,origin:original.origin,current:original.current};original.prior=Array.from({length:16},()=>structuredClone(reference));
      assert.ok(Buffer.byteLength(JSON.stringify(original))>256*1024);
      variant('reference-overflow.opbackup','native.opbackup',manifest=>{manifest.entries=[original];return Buffer.from('captured first');});
      const pageTarget=await page(seed.games,template);native('init','reference-limit',{heads:true});
      const a=native('restore','reference-limit',{archive:'reference-overflow.opbackup'}),b=await webRestore(pageTarget,'reference-overflow.opbackup');
      for(const value of [a,b]){assert.equal(value.result.ok,true);assert.equal(value.result.value.entries[0].error,'resource_limit');assert.deepEqual(value.before,value.after);}await pageTarget.close();
    });
    const malformed=[
      ['extra-fields',(m)=>{m.extra=true;}],['duplicate-save',(m)=>{m.entries.push(m.entries[0]);}],
      ['wrong-payload',(m,payload)=>{const changed=Buffer.from(payload);changed[0]^=1;return changed;}],
      ['legacy-false-missing-parent',(m)=>{m.entries.find(value=>value.origin.kind==='legacy_projection').origin.missing_parent=true;}],
      ['canonical-false-missing-parent',(m)=>{const entry=m.entries.find(value=>value.origin.kind==='captured'&&value.metadata.parent_id===null);entry.origin.missing_parent=true;}],
      ['origin-history-overflow',(m)=>{const entry=m.entries[0];entry.prior=Array.from({length:17},()=>({metadata:entry.metadata,origin:entry.origin,current:entry.current}));}]
    ];
    for(const [name,modify]of malformed)await mark('reject-'+name+'-before-write',async()=>{
      const filename=`invalid-${name}.opbackup`;variant(filename,'native.opbackup',modify);
      const a=native('restore','partial',{archive:filename}),b=await webRestore(q,filename);
      for(const value of [a,b]){assert.equal(value.result.ok,false,`${name} accepted`);assert.deepEqual(value.before,value.after,`${name} changed history`);}
    });
    const badLength=Buffer.from(source.data);badLength.writeUInt32LE(4*1024**2+1,8);
    const invalidUtf8=Buffer.from(source.data);invalidUtf8[12]=255;const signedUtf8=Buffer.concat([invalidUtf8.subarray(0,-32),Buffer.from(sha(Buffer.concat([Buffer.from('Vlikeon portable saves archive v1\0'),invalidUtf8.subarray(0,-32)])),'hex')]);
    for(const [name,data]of [['magic',Buffer.from(source.data)],['truncated',source.data.subarray(0,-1)],['trailer',Buffer.from(source.data)],['trailing',Buffer.concat([source.data,Buffer.from([0])])],['manifest-length',badLength],['utf8',signedUtf8]]){
      if(name==='magic')data[0]^=1;if(name==='trailer')data[data.length-1]^=1;
      await mark('reject-'+name,async()=>{const filename=`bad-${name}.opbackup`;fs.writeFileSync(path.join(work,filename),data);for(const value of [native('restore','partial',{archive:filename}),await webRestore(q,filename)]){assert.equal(value.result.ok,false);assert.deepEqual(value.before,value.after);}});
    }
    for(const name of ['native.opbackup','web.opbackup','web-reexport.opbackup','native-reexport.opbackup'])report.artifacts[name]={sha256:sha(fs.readFileSync(path.join(work,name))),bytes:fs.statSync(path.join(work,name)).size};
    report.status='passed';
  }finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
}
main().catch(error=>{report.status='failed';report.error=error.stack;console.error(error);process.exitCode=1;}).finally(()=>{fs.writeFileSync(path.join(output,'interop-result.json'),JSON.stringify(report,null,2));});
