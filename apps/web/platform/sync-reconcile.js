import{requireValue}from'./errors.js';
import{checkSyncLease}from'./sync-lease.js';
import{assertSyncContext}from'./sync-consent.js';
import{syncRemoval}from'./sync-save-identity.js';
import{validateSave}from'./saves.js';
import{recoverRetainedUpload}from'./sync-upload-recovery.js';

const MAX_UPLOADS=500,MAX_REMOTE_PAGES=20,PAGE=100;
const wipe=buffer=>{if(buffer instanceof ArrayBuffer&&buffer.byteLength)new Uint8Array(buffer).fill(0);};

// Only this authenticated adapter may publish remote metadata. Ciphertext stays
// in the inbox until actual local payload bytes and the canonical origin agree.
export class WebSyncReconciler{
  #store;#saves;#journal;#transport;#credentials;#http;#lease;#context;#progress;#recovery;#recoveryDeferred=0;#busy=false;#waiting=new Set();#deferred=new Set();#summary;
  constructor({store,saves,journal,transport,credentials,http,lease,context,recovery,onProgress=()=>{}}){
    requireValue(typeof onProgress==='function'&&typeof recovery?.process==='function'&&context?.profile_id===lease?.profile,'invalid_request','The synchronization reconciler identity is invalid.');checkSyncLease(lease,lease.profile);this.#store=store;this.#saves=saves;this.#journal=journal;this.#transport=transport;this.#credentials=credentials;this.#http=http;this.#lease=lease;this.#context={...context};this.#progress=onProgress;this.#recovery=recovery;
    this.#summary={uploaded:0,downloaded:0,equivalent:0,suppressed:0,waiting_for_game:0,recovery_deferred:0,pending_copy_recovered:0,cloud_recovered:0,cloud_recovery_intact:0,cloud_recovery_pending:0,cloud_recovery_failed:0,cloud_recovery_cleanup_pending:0,remote_pages:0,more_available:false,transfer:null};
  }
  #guardTx=tx=>assertSyncContext(tx,this.#context);
  async #check(){const signal=checkSyncLease(this.#lease,this.#lease.profile);await this.#store.transaction(['config'],'readonly',this.#guardTx,signal);checkSyncLease(this.#lease,this.#lease.profile);}
  #removed(object){return this.#store.transaction(['config','removed_saves'],'readonly',async tx=>{await this.#guardTx(tx);return syncRemoval(tx,object);},this.#lease.signal);}
  #report(){this.#summary.waiting_for_game=this.#waiting.size;this.#summary.recovery_deferred=this.#deferred.size+this.#recoveryDeferred;try{this.#progress(structuredClone(this.#summary));}catch{}}
  async #apply(item){
    requireValue(item.recovery_request_id===null,'busy','This encrypted inbox copy belongs to an explicit cloud recovery request.');await this.#check();const object=item.object_id;if(await this.#journal.suppressInbox(object)){this.#waiting.delete(object);this.#deferred.delete(object);this.#summary.suppressed++;this.#report();return'suppressed';}
    let encrypted=null,opened=null;
    try{
      encrypted=await item.payload.arrayBuffer();await this.#check();opened=await this.#credentials.open(item.header,encrypted,{signal:this.#lease.signal});await this.#check();const metadata=validateSave(opened.value);
      requireValue(metadata.id===object&&opened.binary instanceof ArrayBuffer&&opened.binary.byteLength===metadata.payload_bytes,'corrupt_data','The authenticated remote save does not match its object identity.');const payload=new Blob([opened.binary],{type:'application/octet-stream'});wipe(opened.binary);
      const saved=await this.#saves.ingestRemote(metadata,payload,{signal:this.#lease.signal,guardTx:this.#guardTx});await this.#check();
      if(!saved){if(await this.#journal.suppressInbox(object)){this.#summary.suppressed++;this.#waiting.delete(object);this.#report();return'suppressed';}this.#waiting.add(object);this.#deferred.delete(object);this.#report();return'waiting';}
      await this.#journal.finishInbox(metadata);this.#waiting.delete(object);this.#deferred.delete(object);this.#summary.downloaded++;this.#summary.transfer=null;this.#report();return'applied';
    }catch(error){if(error.code==='busy'){this.#deferred.add(object);this.#waiting.delete(object);this.#report();return'deferred';}throw error;}
    finally{wipe(encrypted);wipe(opened?.binary);}
  }
  async #drain(){for(const item of await this.#journal.inbox())if(item.recovery_request_id===null)await this.#apply(item);}
  async #upload(id){
    await this.#check();try{await this.#transport.upload(id);this.#summary.uploaded++;this.#summary.transfer=null;this.#report();return true;}
    catch(error){
      if(error.code!=='conflict')throw error;await this.#check();const owner=await this.#journal.uploadOwner();if(owner?.object_id!==id)throw error;
      // Immutable objects can contain the same authenticated generation under
      // another nonce. A header/size or matching HTTP status alone is no proof.
      let item=await this.#journal.inboxObject(id);if(item?.recovery_request_id!==null&&item?.recovery_request_id!==undefined)return false;if(!item){try{item=await this.#transport.download(id);}catch(caught){if(caught.code==='not_found')throw error;throw caught;}}
      const result=await this.#apply(item);if(result!=='applied')return false;this.#summary.uploaded++;this.#summary.equivalent++;this.#report();return true;
    }
  }
  async run(){
    requireValue(!this.#busy,'busy','The save reconciler is already running.');this.#busy=true;
    try{
      await this.#check();await this.#transport.prepare();await this.#check();
      await this.#drain();const downloadOwner=await this.#journal.downloadOwner();if(downloadOwner&&await this.#removed(downloadOwner.object_id))await this.#journal.discardDownload(downloadOwner.id,{removedOnly:true});else if(!downloadOwner||downloadOwner.recovery_request_id===null){const resumed=await this.#transport.resumeDownload();if(resumed)await this.#apply(resumed);}
      const recovery=await this.#recovery.process({saves:this.#saves,journal:this.#journal,transport:this.#transport,credentials:this.#credentials,lease:this.#lease,context:this.#context,onProgress:value=>{const{yielded,recovery_deferred,...counts}=value;this.#recoveryDeferred=recovery_deferred;Object.assign(this.#summary,counts);this.#report();}});if(recovery.cloud_recovery_pending)this.#summary.more_available=true;if(recovery.yielded){this.#summary.more_available=true;this.#report();return structuredClone(this.#summary);}
      const owner=await this.#journal.uploadOwner();if(owner){if(await this.#removed(owner.object_id))await this.#transport.retireUpload(owner.id);else{const recovered=await recoverRetainedUpload({saves:this.#saves,journal:this.#journal,transport:this.#transport,credentials:this.#credentials,lease:this.#lease,context:this.#context});if(recovered.repaired){this.#summary.pending_copy_recovered++;this.#report();}if(!await this.#upload(owner.object_id)){this.#summary.more_available=true;this.#report();return structuredClone(this.#summary);}}}
      let after=null,uploaded=this.#summary.uploaded,scanned=0;
      outgoing:do{
        await this.#check();const page=await this.#saves.syncPage(after,PAGE,{signal:this.#lease.signal,guardTx:this.#guardTx});scanned+=page.items.length;requireValue(scanned<=20000,'resource_limit','This synchronization opportunity reached its local history scan budget.');for(const item of page.items){
          await this.#check();const receipt=await this.#journal.reconcileReceipt(item.id);if(receipt){requireValue(receipt.metadata_hash===item.metadata_hash&&receipt.canonical===item.canonical,'conflict','The local generation disagrees with its immutable synchronization receipt.');continue;}
          if(uploaded>=MAX_UPLOADS){this.#summary.more_available=true;this.#report();break outgoing;}if(!await this.#upload(item.id)){this.#summary.more_available=true;this.#report();return structuredClone(this.#summary);}uploaded++;
        }after=page.after;
      }while(after!==null);
      for(let page=0;page<MAX_REMOTE_PAGES;page++){
        await this.#check();let cursor=await this.#journal.cursor();const rows=await this.#http.list(cursor,PAGE,{signal:this.#lease.signal});await this.#check();this.#summary.remote_pages++;
        for(const item of rows){await this.#check();if(!await this.#journal.advanceKnown(item,cursor)){const held=await this.#journal.inboxObject(item.object_id);if(held?.recovery_request_id!==null&&held?.recovery_request_id!==undefined){this.#summary.more_available=true;this.#report();return structuredClone(this.#summary);}const downloaded=await this.#transport.download(item.object_id,item.sequence,item.encrypted_bytes);await this.#apply(downloaded);}cursor=item.sequence;}
        this.#report();if(rows.length<PAGE)break;if(page===MAX_REMOTE_PAGES-1)this.#summary.more_available=true;
      }
      await this.#drain();this.#summary.cloud_recovery_pending=await this.#recovery.pending(this.#context,{signal:this.#lease.signal});if(this.#summary.cloud_recovery_pending)this.#summary.more_available=true;this.#summary.transfer=null;this.#report();return structuredClone(this.#summary);
    }finally{this.#busy=false;}
  }
}
