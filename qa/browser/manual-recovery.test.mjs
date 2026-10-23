import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import vm from 'node:vm';

// Load the exact authored ES modules without adding a package.json to product
// source. The same modules are included by the product resource generators.
const cache=new Map();
async function load(url){
  url=new URL(url);
  if(url.href.endsWith('/apps/game-hub/runtime/sha256.js'))url=new URL('../../engines/web/sha256.js',import.meta.url);
  if(cache.has(url.href))return cache.get(url.href);
  const module=new vm.SourceTextModule(await readFile(url,'utf8'),{identifier:url.href});
  cache.set(url.href,module); await module.link((specifier,owner)=>load(new URL(specifier,owner.identifier)));
  return module;
}
const format=await load(new URL('../../apps/game-hub/manual-recovery-format.js',import.meta.url));await format.evaluate();
const {inspectRecoveryBlob,packRecovery,unpackRecovery,recoveryRecord}=format.namespace;
const browserSource=await readFile(new URL('../../apps/web/platform/manual-recovery.js',import.meta.url),'utf8');
// The Web product relocates platform/*.js to web/*.js. Its shared format module
// lives beside the Game Hub modules in the assembled bundle.
const web=new vm.SourceTextModule(browserSource,{identifier:new URL('../../apps/web/platform/manual-recovery.js',import.meta.url).href});
await web.link((specifier,owner)=>load(specifier==='../manual-recovery-format.js'?new URL('../../apps/game-hub/manual-recovery-format.js',import.meta.url):specifier==='./sha256.js'?new URL('../../engines/web/sha256.js',import.meta.url):new URL(specifier,owner.identifier)));
await web.evaluate();const {WebManualRecovery}=web.namespace;
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');

// Cursor harness covers inventory/selection policy without claiming real IDB
// transaction or quota qualification (the latter belongs to item 52).
function store(records){
  return {records,transactions:[],async transaction(names,mode,body,signal){
    this.transactions.push({names,mode});
    const tx={store:name=>({openCursor(){const request={};let index=0;
      const deliver=()=>{try{const entry=records[name][index];request.result=entry?{key:entry[0],value:entry[1],continue(){index++;queueMicrotask(deliver);}}:null;request.onsuccess?.();}catch(error){request.error=error;request.onerror?.();}};
      queueMicrotask(deliver);return request;}})};
    return body(tx);
  }};
}

test('recovery package preserves arbitrary bytes and explicit unverified identity',async()=>{
  for(const payload of [Buffer.from([0,255,128,17,10]),Buffer.alloc(0),Buffer.alloc(100000,7)]){
    const blob=new Blob([payload]),record=await inspectRecoveryBlob(blob,'web_saves','unknown α');
    assert.equal(record.trust,'unverified');assert.equal(record.payload_hash,sha(payload));
    const packed=await packRecovery(blob,record),result=await unpackRecovery(packed);
    assert.deepEqual(result.record,record);assert.deepEqual(Buffer.from(await result.payload.arrayBuffer()),payload);
  }
});

test('damaged, truncated, trailing and spoofed provenance packages refuse',async()=>{
  const blob=new Blob(['progress']),record=await inspectRecoveryBlob(blob,'assets','unknown'),packed=Buffer.from(await(await packRecovery(blob,record)).arrayBuffer());
  for(const offset of [0,16,packed.length-33,packed.length-1]){const bad=Buffer.from(packed);bad[offset]^=1;await assert.rejects(unpackRecovery(new Blob([bad])));}
  await assert.rejects(unpackRecovery(new Blob([packed.subarray(0,-1)])));
  await assert.rejects(unpackRecovery(new Blob([packed,Buffer.from([0])])));
  for(const bad of [{...record,trust:'authenticated'},{...record,game_id:'invented'},{...record,payload_bytes:4*1024**3+1},{...record,display_name:'control\nname'}])assert.throws(()=>recoveryRecord(bad));
  await assert.rejects(packRecovery(new Blob(['changed!']),record),{code:'conflict'});
  await assert.rejects(packRecovery(blob,{...record,prefix_hex:'00'.repeat(blob.size)}),{code:'conflict'});
});

test('missing and malformed save metadata do not hide retained payloads',async()=>{
  const payload=new Blob(['lost progress']),records={assets:[['not-a-hash',{blob:new Blob(['content'])}]],saves:[['invalid-save-id',{payload,metadata_json:'broken'}],['unreadable',{payload:'no blob'}]]};
  const db=store(records),service=new WebManualRecovery(db),page=await service.list();
  assert.equal(page.candidates.length,2);assert.equal(page.retained_skipped,1);
  const candidate=page.candidates.find(value=>value.source_kind==='web_saves'),review=await service.inspect(candidate.candidate_id);
  assert.equal(review.record.payload_hash,sha(Buffer.from('lost progress')));
  const result=await service.export(candidate.candidate_id,review.revision),extracted=await unpackRecovery(result.blob);
  assert.equal(await extracted.payload.text(),'lost progress');assert.equal(extracted.record.trust,'unverified');
  assert.equal(records.saves[0][1].payload,payload);assert.ok(db.transactions.every(value=>value.mode==='readonly'));
});

test('same size replacement invalidates review and deleted sources refuse',async()=>{
  const db=store({assets:[['unknown',{blob:new Blob(['one'])}]],saves:[]}),service=new WebManualRecovery(db);
  const id=(await service.list()).candidates[0].candidate_id,review=await service.inspect(id);
  db.records.assets[0][1].blob=new Blob(['two']);
  await assert.rejects(service.export(id,review.revision),{code:'conflict'});
  db.records.assets=[];await assert.rejects(service.inspect(id),{code:'not_found'});
});

test('stable hash cursors page more than 64 records without losing candidates',async()=>{
  const db=store({assets:Array.from({length:67},(_,i)=>['unknown-'+i,{blob:new Blob([])}]),saves:[]}),service=new WebManualRecovery(db);
  const first=await service.list(),second=await service.list(first.next_after);
  assert.equal(first.candidates.length,64);assert.equal(second.candidates.length,3);assert.equal(second.next_after,null);
  assert.equal(new Set([...first.candidates,...second.candidates].map(value=>value.candidate_id)).size,67);
  const controller=new AbortController();controller.abort();await assert.rejects(service.list(null,{signal:controller.signal}),{code:'cancelled'});
  await assert.rejects(service.list('../../path'),{code:'invalid_request'});
});
