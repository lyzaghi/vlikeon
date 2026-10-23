import{MAX_SAVE,UUID,HASH,identifier,integer,utf8,requireValue,checkAbort,fail}from'./errors.js';
import{hashBlob,Sha256}from'./sha256.js';
import{metadataCompatible}from'../compatibility.js';
import{validCoreVersion}from'../runtime/core-resources.js';
import{makeSaveOrigin,checkedSaveOrigin,originMetadataHash,sameSavePayload}from'../save-origin.js';
import{SYNC_SAVE_STORES,syncSaveIdentity,syncSaveMetadata}from'./sync-save-identity.js';
import{hashInWorker}from'./hash-client.js';
import{SYNC_VAULT_RECORD}from'./sync-storage.js';

const FIELDS=['schema_version','id','game_id','game_content_hash','kind','slot','generation','parent_id','core_id','core_version','state_format_version','platform','core_config_hash','payload_hash','payload_bytes','created_at_ms','device_id','label','conflict'];
const validText=(value,max)=>typeof value==='string'&&value.length>0&&utf8(value).length<=max&&Array.from(value).every(c=>{const n=c.codePointAt(0);return n<0xd800||n>0xdfff;});
export function validateSave(input){
  requireValue(input&&typeof input==='object'&&!Array.isArray(input)&&Object.keys(input).every(key=>FIELDS.includes(key)),'invalid_request','Save metadata contains unsupported fields.');
  const value={...input,parent_id:input.parent_id??null,label:input.label??null,conflict:input.conflict===undefined?false:input.conflict};
  requireValue(value.schema_version===1&&[value.id,value.game_id,value.device_id].every(v=>typeof v==='string'&&UUID.test(v))&&(value.parent_id===null||(typeof value.parent_id==='string'&&UUID.test(value.parent_id))),'invalid_request','Save identity is invalid.');
  requireValue(value.parent_id!==value.id,'invalid_request','A save cannot be its own parent.');
  requireValue([value.game_content_hash,value.core_config_hash,value.payload_hash].every(v=>typeof v==='string'&&HASH.test(v)),'invalid_request','Save content hashes are invalid.');
  requireValue(['in_game','emulator_state'].includes(value.kind)&&integer(value.slot,0,65535)&&integer(value.generation,1,Number.MAX_SAFE_INTEGER)&&integer(value.created_at_ms,0,Number.MAX_SAFE_INTEGER)&&integer(value.payload_bytes,1,MAX_SAVE),'invalid_request','Save generation or length is invalid.');
  requireValue(identifier(value.core_id)&&validText(value.core_version,256)&&validCoreVersion(value.core_version)&&validText(value.state_format_version,512)&&validText(value.platform,128)&&(value.label===null||value.label===''||validText(value.label,256))&&typeof value.conflict==='boolean','invalid_request','Save compatibility metadata is invalid.');
  requireValue(utf8(JSON.stringify(value)).length<=64*1024,'resource_limit','Save metadata exceeds its bound.');return value;
}
export function saveMetadata(record){if(!record)fail('not_found','The saved progress was not found.');const{payload,...metadata}=record;return validateSave(metadata);}
export function validateCompatibility(target){
  const fields=['game_content_hash','core_id','core_version','state_format_version','platform','core_config_hash','portable_in_game_format'];
  requireValue(target&&typeof target==='object'&&!Array.isArray(target)&&Object.keys(target).every(key=>fields.includes(key))&&[target.game_content_hash,target.core_config_hash].every(value=>typeof value==='string'&&HASH.test(value))&&identifier(target.core_id)&&validText(target.core_version,256)&&validCoreVersion(target.core_version)&&validText(target.state_format_version,512)&&validText(target.platform,128)&&(target.portable_in_game_format===null||validText(target.portable_in_game_format,512)),'invalid_request','Save compatibility target is invalid.');return target;
}
function cursorValues(request,limit){return new Promise((resolve,reject)=>{const result=[];request.onerror=()=>reject(request.error);request.onsuccess=()=>{const cursor=request.result;if(!cursor||result.length>=limit){resolve(result);return;}result.push(saveMetadata(cursor.value));if(result.length>=limit)resolve(result);else cursor.continue();};});}
export function validateUsage(value){requireValue(value&&['assets_bytes','saves_bytes','save_count','game_count'].every(key=>integer(value[key],0,Number.MAX_SAFE_INTEGER)),'corrupt_data','Browser storage accounting is invalid.');return value;}
function hashFrame(hash,value){const length=new Uint8Array(8);new DataView(length.buffer).setBigUint64(0,BigInt(value.length),true);hash.update(length).update(value);}
const uuidBytes=value=>Uint8Array.from(value.replaceAll('-','').match(/../g),part=>parseInt(part,16));
const numberBytes=value=>{const bytes=new Uint8Array(8);new DataView(bytes.buffer).setBigUint64(0,BigInt(value),true);return bytes;};
async function purgeSnapshot(tx,gameId){
  const game=await tx.get('games',gameId);requireValue(game&&HASH.test(game.content_hash),'not_found','The game was not found.');const hash=new Sha256().update(utf8('omniplay.save-purge-preview.v1\0'));hashFrame(hash,uuidBytes(gameId));hashFrame(hash,utf8(game.content_hash));
  let count=0,bytes=0;const ids=[];
  await new Promise((resolve,reject)=>{const request=tx.store('saves').index('game').openCursor(gameId);request.onerror=()=>reject(request.error);request.onsuccess=()=>{
    try{const cursor=request.result;if(!cursor){resolve();return;}const record=cursor.value,metadata=saveMetadata(record);/* Removal accounts for declared immutable generations, even if their Blob is damaged. */requireValue(integer(bytes+metadata.payload_bytes,0,Number.MAX_SAFE_INTEGER),'resource_limit','Saved history exceeds its accounting range.');hashFrame(hash,utf8(JSON.stringify(metadata)));hashFrame(hash,numberBytes(metadata.payload_bytes));count++;bytes+=metadata.payload_bytes;ids.push(metadata.id);cursor.continue();}catch(error){reject(error);}
  };});
  const heads=await tx.request(tx.store('heads').index('game').getAll(gameId));heads.sort((a,b)=>a.kind<b.kind?-1:a.kind>b.kind?1:a.slot-b.slot);
  for(const head of heads){requireValue(['in_game','emulator_state'].includes(head.kind)&&integer(head.slot,0,65535)&&UUID.test(head.save_id),'corrupt_data','Save heads need recovery.');hashFrame(hash,utf8(head.kind));const slot=new Uint8Array(2);new DataView(slot.buffer).setUint16(0,head.slot,true);hashFrame(hash,slot);hashFrame(hash,utf8(head.save_id));}
  hashFrame(hash,numberBytes(count));hashFrame(hash,numberBytes(heads.length));return{preview:{game_id:gameId,revision:hash.hex(),save_count:count,payload_bytes:bytes,current_heads:heads.length},ids,heads,content_hash:game.content_hash};
}

const sameDomain=(a,b)=>a.game_content_hash===b.game_content_hash&&a.kind===b.kind&&a.slot===b.slot;
async function validateOriginGraph(tx,metadata){
  if(metadata.parent_id!==null){const parent=await tx.get('saves',metadata.parent_id);if(parent)requireValue(sameDomain(saveMetadata(parent),metadata),'content_mismatch','A canonical parent belongs to different saved progress.');}
  let referenced=false,count=0;
  await new Promise((resolve,reject)=>{
    const request=tx.store('save_origins').index('parent').openCursor(IDBKeyRange.only(metadata.id));request.onerror=()=>reject(request.error);
    request.onsuccess=()=>{try{const cursor=request.result;if(!cursor){resolve();return;}requireValue(++count<=20000,'resource_limit','Save ancestry exceeds its bound.');const record=cursor.value,child=validateSave(record.canonical_metadata);requireValue(record.save_id===child.id&&child.parent_id===metadata.id&&record.canonical_hash===originMetadataHash(child),'corrupt_data','Stored save ancestry is inconsistent.');requireValue(sameDomain(child,metadata),'content_mismatch','A canonical child belongs to different saved progress.');referenced=true;cursor.continue();}catch(error){reject(error);}};
  });
  if(!referenced)return;
  const seen=new Set([metadata.id]);let id=metadata.parent_id;
  while(id!==null){
    requireValue(!seen.has(id),'conflict','Canonical save ancestry cannot contain a cycle.');requireValue(seen.size<=20000,'resource_limit','Save ancestry exceeds its bound.');seen.add(id);
    const origin=await tx.get('save_origins',id);if(!origin)break;
    if(origin.kind==='legacy_projection'){requireValue(origin.canonical_metadata===null&&origin.canonical_hash===null,'corrupt_data','Legacy save origin is inconsistent.');break;}
    const previous=validateSave(origin.canonical_metadata);requireValue(previous.id===id&&origin.save_id===id&&origin.canonical_hash===originMetadataHash(previous),'corrupt_data','Stored save ancestry is inconsistent.');id=previous.parent_id;
  }
}

export class WebSaves {
  #store;
  constructor(store){this.#store=store;}
  async syncMetadata(id,{signal,guardTx=null}={}){requireValue(guardTx===null||typeof guardTx==='function','invalid_request','The synchronization metadata guard is invalid.');return this.#store.transaction(['config','sync_slots',...SYNC_SAVE_STORES],'readonly',async tx=>{if(guardTx)await guardTx(tx);return syncSaveMetadata(tx,id);},signal);}
  async syncPage(after=null,limit=100,{signal,guardTx=null}={}){
    requireValue(integer(limit,1,100)&&(guardTx===null||typeof guardTx==='function'),'invalid_request','The outgoing synchronization page is invalid.');
    if(after!==null)requireValue(after&&typeof after==='object'&&!Array.isArray(after)&&Object.keys(after).length===4&&after.schema_version===1&&integer(after.created_at_ms,0,Number.MAX_SAFE_INTEGER)&&integer(after.generation,1,Number.MAX_SAFE_INTEGER)&&typeof after.id==='string'&&UUID.test(after.id),'invalid_request','The outgoing synchronization cursor is invalid.');
    return this.#store.transaction(['config',...SYNC_SAVE_STORES],'readonly',async tx=>{
      if(guardTx)await guardTx(tx);const index=tx.store('saves').index('sync_order');
      if(after===null){const count=await tx.request(tx.store('saves').count());requireValue(count<=20000&&count===await tx.request(index.count()),'corrupt_data','Saved progress indexing needs recovery before synchronization.');}
      const range=after===null?null:IDBKeyRange.lowerBound([after.created_at_ms,after.generation,after.id],true),request=index.openKeyCursor(range),keys=await new Promise((resolve,reject)=>{
        const found=[];request.onerror=()=>reject(request.error);request.onsuccess=()=>{const cursor=request.result;if(!cursor){resolve(found);return;}found.push(cursor.key);if(found.length>limit){resolve(found);return;}cursor.continue();};
      });
      const more=keys.length>limit,selected=keys.slice(0,limit),items=[];for(const key of selected){requireValue(Array.isArray(key)&&key.length===3&&integer(key[0],0,Number.MAX_SAFE_INTEGER)&&integer(key[1],1,Number.MAX_SAFE_INTEGER)&&typeof key[2]==='string'&&UUID.test(key[2]),'corrupt_data','An outgoing saved generation has an invalid ordering key.');const source=await syncSaveMetadata(tx,key[2]);requireValue(source.projection.created_at_ms===key[0]&&source.projection.generation===key[1],'corrupt_data','An outgoing saved generation changed its ordering identity.');items.push({id:key[2],metadata_hash:source.metadata_hash,canonical:source.canonical,payload_revision:source.payload_revision});}
      const last=selected.at(-1);return{items,after:more?{schema_version:1,created_at_ms:last[0],generation:last[1],id:last[2]}:null};
    },signal);
  }
  async forSync(id,{signal}={}){
    const captured=await this.#store.transaction(SYNC_SAVE_STORES,'readonly',tx=>syncSaveIdentity(tx,id),signal);checkAbort(signal);requireValue(await hashInWorker(captured.payload,{signal})===captured.metadata.payload_hash,'corrupt_data','Local saved progress changed before encrypted synchronization.');
    await this.#store.transaction(SYNC_SAVE_STORES,'readonly',async tx=>{const current=await syncSaveIdentity(tx,id);requireValue(current.payload_revision===captured.payload_revision&&current.metadata_hash===captured.metadata_hash&&current.canonical===captured.canonical,'conflict','Saved progress changed while checking its synchronization snapshot.');},signal);checkAbort(signal);return captured;
  }
  async commit(input,payload,parent,{signal,importedFrom=null,remoteMetadata=null,guardTx=null,backupReference=null}={}){
    const metadata=validateSave(input);requireValue(payload instanceof Blob&&payload.size===metadata.payload_bytes&&metadata.parent_id===parent,'content_mismatch','Save bytes or causal parent do not match their metadata.');
    requireValue(guardTx===null||typeof guardTx==='function','invalid_request','The save publication guard is invalid.');checkAbort(signal);requireValue(await(remoteMetadata||backupReference!==null?hashInWorker:hashBlob)(payload,{signal})===metadata.payload_hash,'corrupt_data','Save integrity check failed.');checkAbort(signal);
    requireValue(backupReference===null||typeof backupReference==='string'&&utf8(backupReference).length<=256*1024&&importedFrom!==null&&metadata.conflict===true,'invalid_request','The backup reference is invalid.');
    return this.#store.transaction(['config','games','saves','heads','removed_saves','save_origins','save_payload_revisions','leases'],'readwrite',async tx=>{
      if(guardTx)await guardTx(tx);
      if(backupReference!==null){const leases=await tx.request(tx.store('leases').index('game').getAll(metadata.game_id));requireValue(!leases.some(lease=>!['stopped','failed'].includes(lease.info.state)&&lease.expires_at_ms>Date.now()),'busy','Stop the game before restoring its backup.');}
      requireValue(!(await tx.get('removed_saves',metadata.id)),'conflict','This save identity was intentionally removed locally.');
      const game=await tx.get('games',metadata.game_id);requireValue(game?.content_hash===metadata.game_content_hash,'content_mismatch','Save content is not in this library.');
      requireValue(!(await tx.get('saves',metadata.id)),'conflict','Save identity is already present.');
      const key=[metadata.game_id,metadata.kind,metadata.slot],head=await tx.get('heads',key);
      if(parent!==null){const before=await tx.get('saves',parent);requireValue(before&&before.game_id===metadata.game_id&&before.kind===metadata.kind&&before.slot===metadata.slot,'content_mismatch','The parent belongs to different saved progress.');}
      const range=IDBKeyRange.bound([...key,0],[...key,Number.MAX_SAFE_INTEGER]);
      const greatest=await tx.request(tx.store('saves').index('slot').openCursor(range,'prev'));
      const generation=(greatest?.value.generation??0)+1;requireValue(integer(generation,1,Number.MAX_SAFE_INTEGER),'resource_limit','Save generation limit reached.');
      const usage=validateUsage((await tx.get('config','usage'))?.value);
      requireValue(usage.saves_bytes+payload.size<=8*1024**3&&usage.save_count<20000,'resource_limit','Local save history is full. Export progress before managing browser storage.');
      const saved={...metadata,generation,conflict:metadata.conflict||(head?.save_id??null)!==parent};
      const origin=remoteMetadata?makeSaveOrigin('authenticated_remote',validateSave(remoteMetadata)):makeSaveOrigin(importedFrom?'imported':'captured',saved,importedFrom);
      checkedSaveOrigin(origin,saved);await validateOriginGraph(tx,origin.canonical_metadata);
      await tx.add('saves',{...saved,payload});
      await tx.add('save_payload_revisions',{save_id:saved.id,revision:crypto.randomUUID()});
      await tx.add('save_origins',origin);
      if(backupReference!==null)await tx.add('config',{key:`backup_reference.${saved.id}`,value:backupReference});
      if(!saved.conflict)await tx.put('heads',{game_id:saved.game_id,kind:saved.kind,slot:saved.slot,save_id:saved.id});
      await tx.put('config',{key:'usage',value:{...usage,saves_bytes:usage.saves_bytes+payload.size,save_count:usage.save_count+1}});return saved;
    },signal);
  }
  async list(gameId){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');return this.#store.transaction(['saves'],'readonly',tx=>cursorValues(tx.store('saves').index('history').openCursor(IDBKeyRange.bound([gameId],[gameId,[]]),'prev'),500));}
  async page(gameId,after=null,limit=50){
    requireValue(UUID.test(gameId)&&integer(limit,1,100),'invalid_request','Save history page is invalid.');let cursor=null;
    if(after!==null){
      try{requireValue(typeof after==='string'&&after.length<=1024&&/^[A-Za-z0-9_-]+$/.test(after),'invalid_request','Save history cursor is invalid.');const raw=atob(after.replaceAll('-','+').replaceAll('_','/'));requireValue(btoa(raw).replaceAll('+','-').replaceAll('/','_').replaceAll('=','')===after,'invalid_request','Save history cursor is not canonical.');cursor=JSON.parse(raw);}catch{fail('invalid_request','Save history cursor is invalid.');}
      requireValue(cursor&&Object.keys(cursor).length===5&&cursor.version===1&&cursor.game_id===gameId&&integer(cursor.generation,1,Number.MAX_SAFE_INTEGER)&&integer(cursor.created_at_ms,0,Number.MAX_SAFE_INTEGER)&&UUID.test(cursor.id),'invalid_request','The cursor belongs to another save history.');
    }
    return this.#store.transaction(['games','saves','heads'],'readonly',async tx=>{
      requireValue(await tx.get('games',gameId),'not_found','The game was not found.');const total=await tx.request(tx.store('saves').index('game').count(gameId));
      const range=cursor?IDBKeyRange.bound([gameId],[gameId,cursor.generation,cursor.created_at_ms,cursor.id],false,true):IDBKeyRange.bound([gameId],[gameId,[]]);let size=0,more=false;const records=[];
      await new Promise((resolve,reject)=>{const request=tx.store('saves').index('history').openCursor(range,'prev');request.onerror=()=>reject(request.error);request.onsuccess=()=>{
        try{const item=request.result;if(!item){resolve();return;}const metadata=saveMetadata(item.value),bytes=utf8(JSON.stringify(metadata)).length+128;if(records.length>=limit||size+bytes>256*1024){more=true;resolve();return;}requireValue(metadata.game_id===gameId,'corrupt_data','Save history identity is inconsistent.');size+=bytes;records.push(metadata);item.continue();}catch(error){reject(error);}
      };});
      const entries=[];for(const metadata of records){const head=await tx.get('heads',[gameId,metadata.kind,metadata.slot]);entries.push({metadata,current_head_id:head?.save_id??null});}
      const last=records.at(-1),next_cursor=more&&last?btoa(JSON.stringify({version:1,game_id:gameId,generation:last.generation,created_at_ms:last.created_at_ms,id:last.id})).replaceAll('+','-').replaceAll('/','_').replaceAll('=',''):null;
      return{entries,next_cursor,total};
    });
  }
  async heads(gameId){
    requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');return this.#store.transaction(['heads','saves'],'readonly',async tx=>{const heads=await tx.request(tx.store('heads').index('game').getAll(gameId));const result=[];for(const head of heads)result.push(saveMetadata(await tx.get('saves',head.save_id)));return result;});
  }
  async purgePreview(gameId){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');return this.#store.transaction(['games','saves','heads'],'readonly',async tx=>(await purgeSnapshot(tx,gameId)).preview);}
  async purge(gameId,revision,{signal}={}){
    requireValue(UUID.test(gameId)&&HASH.test(revision),'invalid_request','Removal preview is invalid.');
    return this.#store.transaction(['games','saves','heads','config','leases','removed_saves','save_origins','save_payload_revisions','sync_receipts','sync_recovery'],'readwrite',async tx=>{
      const leases=await tx.request(tx.store('leases').index('game').getAll(gameId));requireValue(!leases.some(lease=>!['stopped','failed'].includes(lease.info.state)&&lease.expires_at_ms>Date.now()),'busy','Stop the game before removing its saved progress.');
      const state=await purgeSnapshot(tx,gameId);requireValue(state.preview.revision===revision,'conflict','Saved progress changed after the preview. Refresh it and choose again.');const usage=validateUsage((await tx.get('config','usage'))?.value);requireValue(usage.saves_bytes>=state.preview.payload_bytes&&usage.save_count>=state.preview.save_count,'corrupt_data','Save accounting needs recovery.');
      const syncAccount=(await tx.get('config',SYNC_VAULT_RECORD))?.value?.account?.account_id;
      for(const id of state.ids){await tx.put('removed_saves',{id,game_content_hash:state.content_hash,removed_at_ms:Date.now()});await tx.delete('save_origins',id);await tx.delete('save_payload_revisions',id);await tx.delete('saves',id);await tx.delete('sync_recovery',id);if(typeof syncAccount==='string'&&HASH.test(syncAccount))await tx.delete('sync_receipts',[syncAccount,id]);}
      for(const head of state.heads)await tx.delete('heads',[gameId,head.kind,head.slot]);
      await tx.put('config',{key:'usage',value:{...usage,saves_bytes:usage.saves_bytes-state.preview.payload_bytes,save_count:usage.save_count-state.preview.save_count}});
      return{game_id:gameId,removed_count:state.preview.save_count,retired_bytes:state.preview.payload_bytes,cleanup:{pending_files:0,pending_bytes:0}};
    },signal);
  }
  async read(id,target=null,{signal}={}){
    requireValue(UUID.test(id),'invalid_request','Save identity is invalid.');const observed=await this.#store.transaction(['saves','save_payload_revisions'],'readonly',async tx=>({record:await tx.get('saves',id),revision:await tx.get('save_payload_revisions',id)}),signal),record=observed.record,metadata=saveMetadata(record);
    if(target){validateCompatibility(target);requireValue(metadata.game_content_hash===target.game_content_hash,'content_mismatch','This save belongs to different content.');requireValue(metadataCompatible(metadata,target),'incompatible_save','Choose a save compatible with the selected runtime.');}
    requireValue(record.payload instanceof Blob&&record.payload.size===metadata.payload_bytes&&await hashBlob(record.payload,{signal})===metadata.payload_hash,'corrupt_data','Stored save integrity check failed.');return{metadata,payload:record.payload,payload_revision:observed.revision?.revision??null};
  }
  async promote(id,expected,{signal}={}){
    requireValue(expected===null||typeof expected==='string'&&UUID.test(expected),'invalid_request','Expected progress identity is invalid.');const{metadata,payload_revision}=await this.read(id,null,{signal});
    requireValue(typeof payload_revision==='string'&&UUID.test(payload_revision),'corrupt_data','Save payload revision needs recovery.');
    return this.#store.transaction(['saves','heads','leases','save_payload_revisions'],'readwrite',async tx=>{
      const leases=await tx.request(tx.store('leases').index('game').getAll(metadata.game_id));requireValue(!leases.some(lease=>!['stopped','failed'].includes(lease.info.state)&&lease.expires_at_ms>Date.now()),'busy','Stop the game before selecting current saved progress.');
      const record=await tx.get('saves',id),revision=await tx.get('save_payload_revisions',id);requireValue(record&&revision?.revision===payload_revision&&originMetadataHash(saveMetadata(record))===originMetadataHash(metadata),'conflict','Saved progress changed. Refresh history and retry.');
      const key=[metadata.game_id,metadata.kind,metadata.slot],head=await tx.get('heads',key);requireValue((head?.save_id??null)===expected,'conflict','Current progress changed. Refresh history before choosing again.');
      const current={...record,conflict:false};await tx.put('saves',current);await tx.put('heads',{game_id:metadata.game_id,kind:metadata.kind,slot:metadata.slot,save_id:id});return saveMetadata(current);
    },signal);
  }
  async origin(id,{signal}={}){
    requireValue(UUID.test(id),'invalid_request','Save identity is invalid.');
    return this.#store.transaction(['saves','save_origins'],'readonly',async tx=>{
      const local=saveMetadata(await tx.get('saves',id)),record=checkedSaveOrigin(await tx.get('save_origins',id),local);let missing=false;
      if(record.canonical_metadata?.parent_id){const parent=await tx.get('saves',record.canonical_metadata.parent_id);missing=!parent||!sameDomain(saveMetadata(parent),local);}
      return checkedSaveOrigin({...record,missing_parent:missing},local);
    },signal);
  }
  async readForSync(id,{signal}={}){const saved=await this.read(id,null,{signal}),origin=await this.origin(id,{signal});return{metadata:origin.canonical_metadata??saved.metadata,payload:saved.payload,canonical:origin.canonical_metadata!==null};}
  async export(id,options){const{metadata,payload}=await this.readForSync(id,options),json=utf8(JSON.stringify(metadata)),header=new Uint8Array(12);header.set(utf8('OPSV0001'));new DataView(header.buffer).setUint32(8,json.length,true);return new Blob([header,json,payload],{type:'application/octet-stream'});}
  // Trusted sync adapters must authenticate/decrypt before calling this method.
  // The sync opportunity supplies a consent/account guard inside each write.
  async ingestRemote(input,payload,{signal,guardTx=null}={}){
    const metadata=validateSave(input);requireValue(payload instanceof Blob&&payload.size===metadata.payload_bytes,'content_mismatch','Remote progress length is invalid.');checkAbort(signal);
    const view=await this.#store.transaction(['config','games','saves','removed_saves'],'readonly',async tx=>{
      if(guardTx)await guardTx(tx);
      if(await tx.get('removed_saves',metadata.id))return null;const game=await tx.request(tx.store('games').index('content_hash').get(metadata.game_content_hash));if(!game)return null;
      const existing=await tx.get('saves',metadata.id),parent=metadata.parent_id?await tx.get('saves',metadata.parent_id):null;return{game,existing,parent};
    },signal);
    if(!view)return null;
    if(view.existing){
      const result=await this.#recoverExisting(metadata,payload,{signal,authenticated:true,guardTx});return result?.metadata??null;
    }
    const parent=view.parent&&sameDomain(saveMetadata(view.parent),metadata)?metadata.parent_id:null;
    return this.commit({...metadata,game_id:view.game.id,parent_id:parent},payload,parent,{signal,remoteMetadata:metadata,guardTx});
  }
  async #decodeExport(blob,{signal}={}){
    checkAbort(signal);requireValue(blob instanceof Blob&&blob.size>=13&&blob.size<=MAX_SAVE+64*1024+12,'corrupt_data','Save export size is invalid.');
    const bytes=new Uint8Array(await blob.slice(0,12).arrayBuffer());requireValue(new TextDecoder().decode(bytes.subarray(0,8))==='OPSV0001','corrupt_data','Unknown saved-progress format.');
    const length=new DataView(bytes.buffer).getUint32(8,true);requireValue(length>0&&length<=64*1024&&blob.size>12+length,'corrupt_data','Save export metadata is truncated.');
    let value;try{value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(await blob.slice(12,12+length).arrayBuffer()));}catch{fail('corrupt_data','Save export metadata is invalid.');}
    const metadata=validateSave(value),payload=blob.slice(12+length);checkAbort(signal);
    requireValue(payload.size===metadata.payload_bytes&&await hashBlob(payload,{signal})===metadata.payload_hash,'corrupt_data','Save export integrity check failed.');return{metadata,payload};
  }
  async repair(id,blob,{signal}={}){
    requireValue(typeof id==='string'&&UUID.test(id),'invalid_request','Save identity is invalid.');
    const{metadata,payload}=await this.#decodeExport(blob,{signal});requireValue(metadata.id===id,'content_mismatch','This recovery file is for a different saved generation.');
    const result=await this.#recoverExisting(metadata,payload,{signal,authenticated:false});requireValue(result!==null,'not_found','This saved generation has been removed.');return result;
  }
  async repairRemote(input,payload,{signal,guardTx,onCommitTx}={}){
    requireValue(typeof guardTx==='function'&&typeof onCommitTx==='function','invalid_request','Authenticated recovery requires request publication guards.');return this.#recoverExisting(input,payload,{signal,authenticated:true,guardTx,onCommitTx});
  }
  async repairStaged(input,payload,{signal,guardTx}={}){requireValue(typeof guardTx==='function','invalid_request','Local staged recovery requires a publication guard.');return this.#recoverExisting(input,payload,{signal,authenticated:false,workerHash:true,guardTx});}
  async #recoverExisting(input,payload,{signal,authenticated=false,workerHash=false,guardTx=null,onCommitTx=null}={}){
    const incoming=validateSave(input);requireValue(payload instanceof Blob&&payload.size===incoming.payload_bytes,'content_mismatch','Recovery payload length is invalid.');
    const hash=authenticated||workerHash?hashInWorker:hashBlob;requireValue(await hash(payload,{signal})===incoming.payload_hash,'corrupt_data','Recovery payload integrity check failed.');checkAbort(signal);
    const observed=await this.#store.transaction(['config','saves','save_origins','removed_saves','save_payload_revisions','sync_recovery','sync_slots'],'readonly',async tx=>{
      if(guardTx)await guardTx(tx);
      if(await tx.get('removed_saves',incoming.id))return null;
      const record=await tx.get('saves',incoming.id),local=saveMetadata(record),origin=checkedSaveOrigin(await tx.get('save_origins',incoming.id),local),revision=await tx.get('save_payload_revisions',incoming.id);
      requireValue(revision?.save_id===local.id&&typeof revision.revision==='string'&&UUID.test(revision.revision),'corrupt_data','Save payload revision needs recovery.');
      requireValue(sameSavePayload(local,incoming),'content_mismatch','This recovery source contains different saved progress.');
      if(authenticated){const proposed=makeSaveOrigin('authenticated_remote',incoming);checkedSaveOrigin(proposed,local);if(origin.canonical_hash)requireValue(origin.canonical_hash===proposed.canonical_hash,'conflict','This save identity has a different canonical origin.');}
      return{record,local,revision:revision.revision};
    },signal);
    if(!observed)return null;
    const original=observed.record.payload,good=original instanceof Blob&&original.size===observed.local.payload_bytes&&await hash(original,{signal})===observed.local.payload_hash;checkAbort(signal);
    return this.#store.transaction(['config','saves','save_origins','removed_saves','save_payload_revisions','leases','sync_recovery','sync_slots'],'readwrite',async tx=>{
      if(guardTx)await guardTx(tx);
      if(await tx.get('removed_saves',incoming.id))return null;
      const record=await tx.get('saves',incoming.id),local=saveMetadata(record),revision=await tx.get('save_payload_revisions',incoming.id);
      requireValue(revision?.revision===observed.revision&&originMetadataHash(local)===originMetadataHash(observed.local),'conflict','Saved progress changed during recovery. Refresh history and retry.');
      const origin=checkedSaveOrigin(await tx.get('save_origins',incoming.id),local);
      if(authenticated){
        const proposed=makeSaveOrigin('authenticated_remote',incoming);checkedSaveOrigin(proposed,local);
        if(origin.canonical_hash)requireValue(origin.canonical_hash===proposed.canonical_hash,'conflict','This save identity has a different canonical origin.');
        else{await validateOriginGraph(tx,incoming);await tx.put('save_origins',proposed);}
      }
      if(!good){
        const leases=await tx.request(tx.store('leases').index('game').getAll(local.game_id));
        requireValue(!leases.some(lease=>!['stopped','failed'].includes(lease.info.state)&&lease.expires_at_ms>Date.now()),'busy','Stop the game before recovering its saved progress.');
        await tx.put('saves',{...record,payload});await tx.put('save_payload_revisions',{save_id:local.id,revision:crypto.randomUUID()});
      }
      const result={metadata:local,repaired:!good};if(onCommitTx)await onCommitTx(tx,result);return result;
    },signal);
  }
  async import(gameId,blob,{signal}={}){
    requireValue(typeof gameId==='string'&&UUID.test(gameId),'invalid_request','Game identity is invalid.');
    const{metadata:previous,payload}=await this.#decodeExport(blob,{signal});
    const metadata={...previous,id:crypto.randomUUID(),game_id:gameId,parent_id:null,generation:1,conflict:false,label:'Imported save'};
    return this.commit(metadata,payload,null,{signal,importedFrom:previous});
  }
}
