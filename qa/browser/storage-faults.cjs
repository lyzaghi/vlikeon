// Real Chromium/IndexedDB fault qualification. No personal browser profile.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.VLIKEON_PLAYWRIGHT_MODULE);
const root=path.resolve(process.env.VLIKEON_WEB_BUNDLE),output=process.env.VLIKEON_QA_OUTPUT;
const report={item:52,scope:'Web IDB upgrades, abort, browser quota and renderer crash',status:'running',cases:[]};
const server=http.createServer((req,res)=>{
  if(req.url==='/fixture.html'){res.setHeader('Content-Type','text/html');res.end('<!doctype html><title>Storage fault fixture</title>');return;}
  const file=path.resolve(root,'.'+decodeURIComponent(req.url.split('?')[0]));if(!file.startsWith(root+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404);res.end();return;}
  res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript':'application/octet-stream');fs.createReadStream(file).pipe(res);
});
async function main(){
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));const origin=`http://127.0.0.1:${server.address().port}`;let browser;
  try{
    browser=await chromium.launch({executablePath:process.env.VLIKEON_CHROME,headless:true});report.browser=await browser.version();
    async function page(){const context=await browser.newContext(),page=await context.newPage();await page.goto(origin+'/fixture.html');return{context,page};}
    async function load(page){await page.evaluate(async()=>{
      const {openStore}=await import('/web/idb.js'),{WebSaves}=await import('/web/saves.js');window.storageOpen=openStore;
      const store=await openStore(),saves=new WebSaves(store);window.api={store,saves,async snapshot(){return store.transaction(['config','saves','heads','save_origins','save_payload_revisions'],'readonly',async tx=>{
        const records=await tx.all('saves');return{config:await tx.all('config'),saves:records.map(({payload,...metadata})=>({...metadata,payload_bytes_observed:payload.size})),heads:await tx.all('heads'),origins:await tx.all('save_origins'),revisions:await tx.all('save_payload_revisions')};
      });}};
    });}
    async function historical(page,version,{invalid=false,keepOpen=false}={}){return page.evaluate(async({version,invalid,keepOpen})=>{
      const id=crypto.randomUUID(),game=crypto.randomUUID(),profile=crypto.randomUUID(),device=crypto.randomUUID(),saveId=invalid?'invalid-id':id;
      const payloadHash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode('old progress'))),byte=>byte.toString(16).padStart(2,'0')).join('');
      const metadata={schema_version:1,id:saveId,game_id:game,game_content_hash:'a'.repeat(64),kind:'in_game',slot:0,generation:1,parent_id:null,core_id:'storage-fixture',core_version:'1.0.0',state_format_version:'fixture-v1',platform:'fixture',core_config_hash:'b'.repeat(64),payload_hash:payloadHash,payload_bytes:12,created_at_ms:1,device_id:device,label:'Preserve 🎮',conflict:false};
      const request=indexedDB.open('omniplay-local-v1',version);request.onupgradeneeded=()=>{
        const db=request.result,config=db.createObjectStore('config',{keyPath:'key'}),games=db.createObjectStore('games',{keyPath:'id'});games.createIndex('content_hash','content_hash',{unique:true});games.createIndex('sort',['sort_title','id'],{unique:true});games.createIndex('system_sort',['system','sort_title','id'],{unique:true});
        if(version>=2){games.createIndex('favorite_sort',['favorite','sort_title','id'],{unique:true});games.createIndex('system_favorite_sort',['system','favorite','sort_title','id'],{unique:true});}
        db.createObjectStore('assets',{keyPath:'hash'});const saves=db.createObjectStore('saves',{keyPath:'id'});saves.createIndex('game','game_id');saves.createIndex('slot',['game_id','kind','slot','generation'],{unique:true});saves.createIndex('history',['game_id','generation','created_at_ms','id'],{unique:true});if(version>=8)saves.createIndex('sync_order',['created_at_ms','generation','id'],{unique:true});
        const heads=db.createObjectStore('heads',{keyPath:['game_id','kind','slot']});heads.createIndex('game','game_id');const runs=db.createObjectStore('performance',{keyPath:'id'});runs.createIndex('game','game_id');runs.createIndex('created','created_at_ms');const leases=db.createObjectStore('leases',{keyPath:'id'});leases.createIndex('game','game_id');
        if(version>=3)db.createObjectStore('removed_saves',{keyPath:'id'});
        if(version>=4){db.createObjectStore('firmware_metadata',{keyPath:'content_hash'});db.createObjectStore('firmware_data',{keyPath:'content_hash'});db.createObjectStore('firmware_selections',{keyPath:'key'}).createIndex('hash','content_hash');}
        if(version>=5){const origins=db.createObjectStore('save_origins',{keyPath:'save_id'});origins.createIndex('parent','canonical_metadata.parent_id');origins.add({schema_version:1,save_id:saveId,kind:'legacy_projection',canonical_metadata:null,canonical_hash:null,imported_from:null,imported_from_hash:null,missing_parent:false});}
        if(version>=6)db.createObjectStore('save_payload_revisions',{keyPath:'save_id'}).add({save_id:saveId,revision:crypto.randomUUID()});
        if(version>=7){const slots=db.createObjectStore('sync_slots',{keyPath:'direction'});slots.add({direction:'upload',schema_version:1,fixture:'retain slot'});db.createObjectStore('sync_parts',{keyPath:['transfer_id','offset']}).createIndex('transfer','transfer_id');db.createObjectStore('sync_inbox',{keyPath:['account_id','object_id']}).createIndex('account','account_id');db.createObjectStore('sync_receipts',{keyPath:['account_id','object_id']}).createIndex('account','account_id');}
        config.add({key:'profile_id',value:profile});config.add({key:'device_id',value:device});config.add({key:'usage',value:{assets_bytes:0,saves_bytes:12,save_count:1,game_count:1}});config.add({key:'sentinel',value:'preserve settings'});
        games.add({id:game,content_hash:'a'.repeat(64),title:'Fixture',sort_title:'fixture',system:'nes',...(version>=2?{favorite:1}:{})});saves.add({...metadata,payload:new Blob(['old progress'])});heads.add({game_id:game,kind:'in_game',slot:0,save_id:saveId});
      };
      const db=await new Promise((resolve,reject)=>{request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});
      if(keepOpen)window.oldConnection=db;else db.close();return{id:saveId,game,profile,device};
    },{version,invalid,keepOpen});}
    async function mark(id,body){await body();report.cases.push({id,status:'passed'});fs.writeFileSync(path.join(output,'web-fault-progress.json'),JSON.stringify(report,null,2));}
    await mark('fresh-idb-schema-nine',async()=>{const p=await page();try{await load(p.page);const value=await p.page.evaluate(()=>api.snapshot());assert.equal(value.saves.length,0);}finally{await p.context.close();}});
    for(let version=1;version<=8;version++)await mark(`idb-${version}-to-nine-preserves-data`,async()=>{const p=await page();try{
      const seed=await historical(p.page,version);await load(p.page);const result=await p.page.evaluate(async()=>{const value=await api.snapshot();return{snapshot:value,payload:await(await api.store.transaction(['saves'],'readonly',async tx=>(await tx.all('saves'))[0])).payload.text(),slots:await api.store.transaction(['sync_slots'],'readonly',tx=>tx.all('sync_slots')),dbs:await indexedDB.databases()};});
      assert.equal(result.dbs[0].version,9);assert.equal(result.snapshot.saves[0].id,seed.id);assert.equal(result.snapshot.heads[0].save_id,seed.id);assert.equal(result.payload,'old progress');assert.equal(result.snapshot.config.find(value=>value.key==='device_id').value,seed.device);assert.equal(result.snapshot.config.find(value=>value.key==='profile_id').value,seed.profile);assert.equal(result.snapshot.config.find(value=>value.key==='sentinel').value,'preserve settings');assert.equal(result.snapshot.origins[0].kind,'legacy_projection');assert.match(result.snapshot.revisions[0].revision,/^[a-f0-9-]{36}$/);if(version>=7){assert.equal(result.slots[0].fixture,'retain slot');assert.equal(result.slots[0].recovery_request_id,null);}
    }finally{await p.context.close();}});
    await mark('migration-abort-retains-old-schema-and-blob',async()=>{const p=await page();try{
      await historical(p.page,4,{invalid:true});const result=await p.page.evaluate(async()=>{const {openStore}=await import('/web/idb.js');let error=null;try{await openStore();}catch(value){error=value.code;}const request=indexedDB.open('omniplay-local-v1'),db=await new Promise((resolve,reject)=>{request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});const value=await new Promise((resolve,reject)=>{const tx=db.transaction('saves'),request=tx.objectStore('saves').get('invalid-id');request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});const result={error,version:db.version,originStore:db.objectStoreNames.contains('save_origins'),payload:await value.payload.text()};db.close();return result;});assert.ok(result.error);assert.equal(result.version,4);assert.equal(result.originStore,false);assert.equal(result.payload,'old progress');
    }finally{await p.context.close();}});
    await mark('blocked-upgrade-rejects-and-retries-after-old-tab-close',async()=>{const p=await page();try{
      await historical(p.page,1,{keepOpen:true});const result=await p.page.evaluate(async()=>{const {openStore}=await import('/web/idb.js');const notices=[];let error;try{await openStore(value=>notices.push(value));}catch(value){error=value.code;}oldConnection.close();await new Promise(resolve=>setTimeout(resolve,100));const store=await openStore(),count=await store.transaction(['saves'],'readonly',async tx=>(await tx.all('saves')).length);store.close();return{error,notices,count};});assert.equal(result.error,'busy');assert.ok(result.notices.includes('blocked'));assert.equal(result.count,1);
    }finally{await p.context.close();}});
    await mark('version-change-closes-old-store-and-future-schema-is-preserved',async()=>{const p=await page();try{
      await historical(p.page,8);await load(p.page);const before=await p.page.evaluate(()=>api.snapshot());
      const value=await p.page.evaluate(async()=>{
        const request=indexedDB.open('omniplay-local-v1',10),future=await new Promise((resolve,reject)=>{request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});
        let oldError=null;try{await api.store.config('device_id');}catch(error){oldError=error.code;}future.close();
        let futureError=null;try{await storageOpen();}catch(error){futureError=error.code;}
        const raw=indexedDB.open('omniplay-local-v1'),db=await new Promise((resolve,reject)=>{raw.onsuccess=()=>resolve(raw.result);raw.onerror=()=>reject(raw.error);});
        const tx=db.transaction(['saves','heads']),saves=await new Promise(resolve=>{const value=tx.objectStore('saves').getAll();value.onsuccess=()=>resolve(value.result);});const result={oldError,futureError,version:db.version,save_id:saves[0].id,payload:await saves[0].payload.text()};db.close();return result;
      });assert.equal(value.oldError,'storage_failure');assert.equal(value.futureError,'storage_failure');assert.equal(value.version,10);assert.equal(value.save_id,before.saves[0].id);assert.equal(value.payload,'old progress');
    }finally{await p.context.close();}});
    async function current(page){await load(page);return page.evaluate(async()=>{
      const {Sha256}=await import('/runtime/sha256.js'),game=crypto.randomUUID(),device=await api.store.config('device_id'),payload=new Blob(['old progress']);
      const template={schema_version:1,id:crypto.randomUUID(),game_id:game,game_content_hash:'a'.repeat(64),kind:'in_game',slot:0,generation:1,parent_id:null,core_id:'storage-fixture',core_version:'1.0.0',state_format_version:'fixture-v1',platform:'fixture',core_config_hash:'b'.repeat(64),payload_hash:new Sha256().update(new TextEncoder().encode('old progress')).hex(),payload_bytes:payload.size,created_at_ms:1,device_id:device,label:null,conflict:false};
      await api.store.transaction(['games','config'],'readwrite',async tx=>{await tx.add('games',{id:game,content_hash:template.game_content_hash,title:'Fixture',sort_title:'fixture',system:'nes',favorite:0});const usage=await tx.get('config','usage');await tx.put('config',{key:'usage',value:{...usage.value,game_count:1}});});await api.saves.commit(template,payload,null);api.template=template;return api.snapshot();
    });}
    await mark('abort-real-save-publication-rolls-back-all-stores',async()=>{const p=await page();try{
      const before=await current(p.page);const value=await p.page.evaluate(async()=>{
        const original=IDBObjectStore.prototype.add,controller=new AbortController();let observed=false;IDBObjectStore.prototype.add=function(...args){const request=original.apply(this,args);if(this.name==='saves')request.addEventListener('success',()=>{observed=true;controller.abort();},{once:true});return request;};let error;
        try{await api.saves.commit({...api.template,id:crypto.randomUUID(),parent_id:api.template.id},new Blob(['old progress']),api.template.id,{signal:controller.signal});}catch(value){error=value.code;}finally{IDBObjectStore.prototype.add=original;}return{error,observed,after:await api.snapshot()};
      });assert.equal(value.error,'cancelled');assert.equal(value.observed,true);assert.deepEqual(value.after,before);
    }finally{await p.context.close();}});
    await mark('real-browser-quota-error-preserves-save-and-retries',async()=>{const p=await page();try{
      // Apply before opening IDB: Chromium can cache a prior space grant in
      // the bucket, so lowering quota after seeding need not reject this write.
      const session=await p.context.newCDPSession(p.page);await session.send('Storage.overrideQuotaForOrigin',{origin,quotaSize:5*1024**2});
      const quota=await session.send('Storage.getUsageAndQuota',{origin});assert.equal(quota.quota,5*1024**2);assert.equal(quota.overrideActive,true);
      report.quota_before_seed=quota;const before=await current(p.page);
      const value=await p.page.evaluate(async()=>{
        const bytes=new Uint8Array(8*1024**2);crypto.getRandomValues(bytes.subarray(0,65536));for(let offset=65536;offset<bytes.length;offset+=65536)bytes.set(bytes.subarray(0,65536),offset);
        const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),value=>value.toString(16).padStart(2,'0')).join(''),id=crypto.randomUUID();api.quotaRetry={metadata:{...api.template,id,parent_id:api.template.id,payload_bytes:bytes.length,payload_hash:hash},payload:new Blob([bytes])};
        const original=IDBDatabase.prototype.transaction,raw=[];IDBDatabase.prototype.transaction=function(...args){const tx=original.apply(this,args);tx.addEventListener('abort',()=>raw.push(tx.error?.name));return tx;};let error;
        try{await api.saves.commit(api.quotaRetry.metadata,api.quotaRetry.payload,api.template.id);}catch(value){error=value.code;}finally{IDBDatabase.prototype.transaction=original;}return{error,raw,after:await api.snapshot()};
      });assert.equal(value.error,'resource_limit');assert.ok(value.raw.includes('QuotaExceededError'));assert.deepEqual(value.after,before);await session.send('Storage.overrideQuotaForOrigin',{origin});
      const retry=await p.page.evaluate(async()=>{await api.saves.commit(api.quotaRetry.metadata,api.quotaRetry.payload,api.template.id);return api.snapshot();});assert.equal(retry.saves.length,2);assert.equal(retry.heads[0].save_id,retry.saves.find(value=>value.id!==before.saves[0].id).id);
    }finally{await p.context.close();}});
    for(const committed of [false,true])await mark(committed?'renderer-crash-after-commit-retains-write':'renderer-crash-before-commit-rolls-back',async()=>{const p=await page();try{
      const before=await current(p.page);let notify;const ready=new Promise(resolve=>notify=resolve);await p.page.exposeFunction('phaseReady',notify);
      const saveId=await p.page.evaluate(committed=>{
        const id=crypto.randomUUID();
        if(!committed){const original=IDBObjectStore.prototype.put;IDBObjectStore.prototype.put=function(...args){const request=original.apply(this,args);if(this.name==='heads')request.addEventListener('success',()=>{
          // Hold this actual save transaction open after head publication. No
          // timer/network await is inserted into the product transaction body.
          const config=this.transaction.objectStore('config');function hold(){const next=config.get('device_id');next.onsuccess=hold;}hold();void phaseReady();
        },{once:true});return request;};}
        window.pending=api.saves.commit({...api.template,id,parent_id:api.template.id},new Blob(['old progress']),api.template.id);
        if(committed)void pending.then(()=>phaseReady());void pending.catch(()=>{});return id;
      },committed);
      await ready;const session=await p.context.newCDPSession(p.page),crashed=p.page.waitForEvent('crash',{timeout:10000});void session.send('Page.crash').catch(()=>{});await crashed;
      const next=await p.context.newPage();await next.goto(origin+'/fixture.html');await load(next);const after=await next.evaluate(()=>api.snapshot());
      if(committed){assert.equal(after.saves.length,2);assert.equal(after.heads[0].save_id,saveId);assert.equal(after.origins.length,2);assert.equal(after.revisions.length,2);assert.equal(after.config.find(value=>value.key==='usage').value.save_count,2);const payload=await next.evaluate(async id=>(await api.saves.read(id)).payload.text(),saveId);assert.equal(payload,'old progress');}
      else assert.deepEqual(after,before);
    }finally{await p.context.close();}});
    report.status='passed';
  }finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
}
main().catch(error=>{report.status='failed';report.error=error.stack;console.error(error);process.exitCode=1;}).finally(()=>fs.writeFileSync(path.join(output,'web-fault-result.json'),JSON.stringify(report,null,2)));
