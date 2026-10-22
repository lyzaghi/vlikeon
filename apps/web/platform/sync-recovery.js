import{checkAbort,requireValue,UUID,HASH}from'./errors.js';
import{assertSyncContext,loadSyncContext}from'./sync-consent.js';
import{SYNC_VAULT_RECORD}from'./sync-storage.js';
import{SYNC_SAVE_STORES,syncSaveMetadata}from'./sync-save-identity.js';
import{RECOVERY_LIMIT,recoveryEntries,recoveryRecord,recoveryTerminal,assertRecoveryRequest,assertRecoveryLocal,setRecoveryState}from'./sync-recovery-state.js';
import{originMetadataHash}from'../save-origin.js';
import{validateSave}from'./saves.js';
import{hashInWorker}from'./hash-client.js';
import{checkSyncLease}from'./sync-lease.js';

const uuid=value=>typeof value==='string'&&UUID.test(value);
const wipe=buffer=>{if(buffer instanceof ArrayBuffer&&buffer.byteLength)new Uint8Array(buffer).fill(0);};
const codes=new Set(['invalid_request','unsupported_capability','unsupported_protocol','permission_denied','permission_expired','not_found','content_mismatch','conflict','resource_limit','cancelled','deadline_exceeded','network_unavailable','storage_failure','corrupt_data','untrusted_artifact','busy']);
function failure(error){const code=codes.has(error?.code)?error.code:'storage_failure';return{code,message:code==='not_found'?'This account has no available copy of this saved generation.':['conflict','content_mismatch','corrupt_data'].includes(code)?'The recovery source or local metadata does not match this saved generation.':'Cloud recovery could not finish. Existing local progress remains available.',recoverable:true};}
const summary=()=>({cloud_recovered:0,cloud_recovery_intact:0,cloud_recovery_pending:0,cloud_recovery_failed:0,cloud_recovery_cleanup_pending:0,recovery_deferred:0,yielded:false});

export class WebSyncRecovery{
  #store;#profile;#available;
  constructor(store,profile,{available=()=>false}={}){requireValue(uuid(profile)&&typeof available==='function','invalid_request','The cloud recovery profile is invalid.');this.#store=store;this.#profile=profile;this.#available=available;}
  async #account(tx){const vault=(await tx.get('config',SYNC_VAULT_RECORD))?.value;if(!vault?.connected)return null;requireValue(vault.profile_id===this.#profile&&typeof vault.account?.account_id==='string'&&HASH.test(vault.account.account_id)&&uuid(vault.account.device_id),'corrupt_data','The connected cloud recovery identity is malformed.');return vault.account;}
  async request(id,{signal}={}){
    requireValue(uuid(id),'invalid_request','The cloud recovery save identity is invalid.');requireValue(this.#available(),'unsupported_capability','Encrypted cloud recovery is unavailable.');requireValue(!document.hidden&&document.hasFocus(),'cancelled','Request cloud recovery from the foreground Game Hub.');const context=await loadSyncContext(this.#store,this.#profile,{signal});
    return this.#store.transaction(['config','sync_recovery',...SYNC_SAVE_STORES],'readwrite',async tx=>{
      await assertSyncContext(tx,context);const source=await syncSaveMetadata(tx,id),all=await recoveryEntries(tx,context.account_id),previous=await tx.get('sync_recovery',id),count=await tx.request(tx.store('sync_recovery').count());if(previous){const row=recoveryRecord(previous,id);requireValue(row.account_id===context.account_id&&row.device_id===context.device_id,'conflict','A different account owns this recovery request.');}
      if(!previous&&count>=RECOVERY_LIMIT){const old=all.find(value=>recoveryTerminal(value));requireValue(old,'resource_limit','Finish or cancel an existing cloud recovery request first.');await tx.delete('sync_recovery',old.save_id);}
      const now=Date.now(),entry={schema_version:1,save_id:id,request_id:crypto.randomUUID(),account_id:context.account_id,device_id:context.device_id,game_id:source.projection.game_id,local_metadata_hash:originMetadataHash(source.projection),canonical_hash:source.canonical?source.metadata_hash:null,completion:null,status:{save_id:id,state:'queued',requested_at_ms:now,updated_at_ms:now,error:null}};recoveryRecord(entry);await tx.put('sync_recovery',entry);return entry.status;
    },signal);
  }
  async list(game,{signal}={}){requireValue(uuid(game),'invalid_request','The cloud recovery game identity is invalid.');return this.#store.transaction(['config','sync_recovery'],'readonly',async tx=>{const account=await this.#account(tx);if(!account)return[];return(await recoveryEntries(tx,account.account_id)).filter(value=>value.game_id===game&&value.device_id===account.device_id).map(value=>value.status);},signal);}
  async cancel(id,{signal}={}){requireValue(uuid(id),'invalid_request','The cloud recovery save identity is invalid.');return this.#store.transaction(['config','sync_recovery'],'readwrite',async tx=>{const account=await this.#account(tx),raw=await tx.get('sync_recovery',id);requireValue(account&&raw,'not_found','The cloud recovery request was not found.');const value=recoveryRecord(raw,id);requireValue(value.account_id===account.account_id&&value.device_id===account.device_id,'permission_expired','The cloud recovery account changed.');if(recoveryTerminal(value))return value.status;return(await setRecoveryState(tx,value,'cancelled')).status;},signal);}
  async pending(context,{signal}={}){return this.#store.transaction(['config','sync_recovery'],'readonly',async tx=>{await assertSyncContext(tx,context);return(await recoveryEntries(tx,context.account_id)).filter(value=>value.device_id===context.device_id&&!recoveryTerminal(value)).length;},signal);}
  async process({saves,journal,transport,credentials,lease,context,onProgress=()=>{}}){
    requireValue(typeof onProgress==='function'&&lease.profile===this.#profile&&context.profile_id===this.#profile,'invalid_request','The cloud recovery opportunity is invalid.');const signal=checkSyncLease(lease,this.#profile),result=summary();
    const report=()=>{try{onProgress({...result});}catch{}};
    const entries=()=>this.#store.transaction(['config','sync_recovery'],'readonly',async tx=>{await assertSyncContext(tx,context);const all=await recoveryEntries(tx,context.account_id);requireValue(all.every(value=>value.device_id===context.device_id),'conflict','Retained recovery requests belong to a different device.');return all;},signal);
    const guardTx=entry=>async tx=>{await assertSyncContext(tx,context);await assertRecoveryRequest(tx,entry.request_id,entry.save_id,context);return assertRecoveryLocal(tx,entry);};
    const guard=entry=>async()=>{checkSyncLease(lease,this.#profile);await this.#store.transaction(['config','sync_recovery',...SYNC_SAVE_STORES],'readonly',guardTx(entry),signal);checkSyncLease(lease,this.#profile);};
    const update=(entry,state,error=null)=>this.#store.transaction(['config','sync_recovery'],'readwrite',async tx=>{await assertSyncContext(tx,context);return setRecoveryState(tx,entry,state,error);},signal);
    const pendingCount=async()=>{result.cloud_recovery_pending=(await entries()).filter(value=>!recoveryTerminal(value)).length;report();};
    const cleanup=async(request,object)=>{try{await journal.retireRecovery(request,object);}catch(error){result.cloud_recovery_cleanup_pending++;report();throw error;}};
    let all=await entries(),owner=await journal.downloadOwner();const retained=await journal.inbox();
    for(const copy of[...(owner?.recovery_request_id?[{object_id:owner.object_id,recovery_request_id:owner.recovery_request_id}]:[]),...retained.filter(value=>value.recovery_request_id!==null)]){
      const replacement=all.find(value=>value.save_id===copy.object_id&&!recoveryTerminal(value)&&value.request_id!==copy.recovery_request_id);if(replacement){await journal.claimRecovery(replacement.request_id,replacement.save_id);continue;}
      const entry=all.find(value=>value.save_id===copy.object_id&&value.request_id===copy.recovery_request_id);if(entry?.completion&&copy.header){requireValue(copy.header.ciphertext_sha256===entry.completion.ciphertext_sha256,'conflict','The completed recovery inbox copy changed before acknowledgement.');const source=await this.#store.transaction(['config',...SYNC_SAVE_STORES],'readonly',async tx=>{await assertSyncContext(tx,context);return syncSaveMetadata(tx,entry.save_id);},signal);requireValue(source.canonical&&source.metadata_hash===entry.completion.canonical_metadata_hash,'conflict','The completed recovered origin changed before acknowledgement.');await journal.finishInbox(source.metadata,{recoveryRequest:entry.request_id});}
      else if(!entry||recoveryTerminal(entry)){await cleanup(copy.recovery_request_id,copy.object_id);if(entry?.status.state==='cancelled')result.yielded=true;}
    }
    owner=await journal.downloadOwner();all=await entries();const pending=all.filter(value=>!recoveryTerminal(value));pending.sort((a,b)=>Number(b.save_id===owner?.object_id)-Number(a.save_id===owner?.object_id)||a.status.updated_at_ms-b.status.updated_at_ms||a.status.requested_at_ms-b.status.requested_at_ms||a.save_id.localeCompare(b.save_id));result.cloud_recovery_pending=pending.length;report();if(result.yielded)return{...result};
    for(const entry of pending.slice(0,8)){
      let encrypted=null,opened=null;
      try{
        const before=await this.#store.transaction(['config','sync_recovery',...SYNC_SAVE_STORES],'readonly',guardTx(entry),signal),intact=before.payload instanceof Blob&&before.payload.size===before.projection.payload_bytes&&await hashInWorker(before.payload,{signal})===before.projection.payload_hash;await guard(entry)();
        if(intact){
          await this.#store.transaction(['config','sync_recovery',...SYNC_SAVE_STORES],'readwrite',async tx=>{const current=await guardTx(entry)(tx);requireValue(current.payload_revision===before.payload_revision&&current.canonical===before.canonical&&current.metadata_hash===before.metadata_hash,'conflict','Saved progress changed while its recovery health was checked.');await setRecoveryState(tx,entry,'already_intact');},signal);result.cloud_recovery_intact++;await cleanup(entry.request_id,entry.save_id);await pendingCount();continue;
        }
        const active=await this.#store.transaction(['config','sync_recovery',...SYNC_SAVE_STORES,'leases'],'readwrite',async tx=>{await guardTx(entry)(tx);const leases=await tx.request(tx.store('leases').index('game').getAll(entry.game_id)),busy=leases.some(value=>!['stopped','failed'].includes(value.info.state)&&value.expires_at_ms>Date.now());await setRecoveryState(tx,entry,busy?'waiting_for_game_close':'downloading');return busy;},signal);
        if(active){result.recovery_deferred++;await pendingCount();const held=await journal.downloadOwner();if(held?.object_id===entry.save_id){result.yielded=true;break;}continue;}
        await journal.claimRecovery(entry.request_id,entry.save_id);let item=await journal.inboxObject(entry.save_id);if(!item)item=await transport.download(entry.save_id,null,null,{recoveryRequest:entry.request_id,guard:guard(entry)});await guard(entry)();requireValue(item.recovery_request_id===entry.request_id,'conflict','The recovery inbox owner changed.');
        encrypted=await item.payload.arrayBuffer();await guard(entry)();opened=await credentials.open(item.header,encrypted,{signal});await guard(entry)();const metadata=validateSave(opened.value);requireValue(metadata.id===entry.save_id&&opened.binary instanceof ArrayBuffer&&opened.binary.byteLength===metadata.payload_bytes,'content_mismatch','The recovery copy belongs to different saved progress.');const payload=new Blob([opened.binary],{type:'application/octet-stream'});wipe(opened.binary);
        const completion={canonical_metadata_hash:originMetadataHash(metadata),ciphertext_sha256:item.header.ciphertext_sha256},outcome=await saves.repairRemote(metadata,payload,{signal,guardTx:guardTx(entry),onCommitTx:async(tx,value)=>{await setRecoveryState(tx,entry,value.repaired?'recovered':'already_intact',null,completion);}});requireValue(outcome,'not_found','This saved generation was removed during recovery.');if(outcome.repaired)result.cloud_recovered++;else result.cloud_recovery_intact++;report();
        await journal.finishInbox(metadata,{recoveryRequest:entry.request_id});await pendingCount();
      }catch(error){
        checkAbort(signal);if(!journal.usable)throw error;
        const current=await this.#store.transaction(['config','sync_recovery'],'readonly',async tx=>{await assertSyncContext(tx,context);const raw=await tx.get('sync_recovery',entry.save_id);return raw?recoveryRecord(raw,entry.save_id):null;},signal);
        if(!current||current.request_id!==entry.request_id||recoveryTerminal(current)){if(current?.request_id===entry.request_id&&recoveryTerminal(current)){if(current.completion)throw error;await cleanup(entry.request_id,entry.save_id);if(current.status.state==='cancelled')result.yielded=true;}await pendingCount();if(result.yielded)break;continue;}
        if(error.code==='busy'){await update(entry,'waiting_for_game_close');result.recovery_deferred++;await pendingCount();const held=await journal.downloadOwner();if(held?.object_id===entry.save_id){result.yielded=true;break;}continue;}
        if(['network_unavailable','deadline_exceeded'].includes(error.code)){await update(entry,'queued',failure(error));await pendingCount();throw error;}
        if(error.code==='cancelled')throw error;
        await update(entry,'failed',failure(error));result.cloud_recovery_failed++;await cleanup(entry.request_id,entry.save_id);await pendingCount();
      }finally{wipe(encrypted);wipe(opened?.binary);}
    }
    await pendingCount();return{...result};
  }
}
