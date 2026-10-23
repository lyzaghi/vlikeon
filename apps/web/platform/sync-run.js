import{checkAbort,requireValue,UUID}from'./errors.js';
import{readSyncNetworkPolicy}from'./sync-network-policy.js';
import{assertSyncContext,loadSyncContext,syncConsentStatus,configureSyncConsent,loadLocalExportContext}from'./sync-consent.js';
import{validateSettings,defaultSettings}from'./preferences.js';
import{withSyncLease}from'./sync-lease.js';
import{WebSyncHttp}from'./sync-http.js';
import{WebSyncJournal}from'./sync-journal.js';
import{WebSyncResumable}from'./sync-resumable.js';
import{WebSyncReconciler}from'./sync-reconcile.js';
import{exportRetainedUpload}from'./sync-upload-recovery.js';

const RUN_TIME=5*60*1000;
const codes=new Set(['invalid_request','unsupported_capability','unsupported_protocol','permission_denied','permission_expired','not_found','content_mismatch','conflict','resource_limit','cancelled','deadline_exceeded','network_unavailable','storage_failure','corrupt_data','untrusted_artifact','busy']);
const emptySummary=()=>({uploaded:0,downloaded:0,equivalent:0,suppressed:0,waiting_for_game:0,recovery_deferred:0,pending_copy_recovered:0,cloud_recovered:0,cloud_recovery_intact:0,cloud_recovery_pending:0,cloud_recovery_failed:0,cloud_recovery_cleanup_pending:0,remote_pages:0,more_available:false,transfer:null});
function failure(error,current){const code=current.timedOut?'deadline_exceeded':current.controller.signal.aborted?'cancelled':codes.has(error?.code)?error.code:'storage_failure';return{code,message:code==='cancelled'?'Synchronization stopped. Retained encrypted transfers can resume.':code==='deadline_exceeded'?'This synchronization opportunity reached its deadline. Choose Sync now to continue.':code==='conflict'?'Saved progress identities differ. Both copies remain retained for recovery.':'Synchronization could not finish. Local progress and retained encrypted copies remain available.',recoverable:true};}

export class WebSyncRun{
  #store;#saves;#credentials;#account;#profile;#recovery;#available=false;#current=null;#starting=false;#startController=null;#startTask=null;#closePromise=null;#closed=false;#listeners=new AbortController();#exportController=null;#exportTask=null;
  constructor(store,saves,credentials,account,profile,{recovery}={}){
    requireValue(typeof profile==='string'&&UUID.test(profile)&&typeof recovery?.process==='function','invalid_request','The browser synchronization profile is invalid.');this.#store=store;this.#saves=saves;this.#credentials=credentials;this.#account=account;this.#profile=profile;this.#recovery=recovery;
    const options={signal:this.#listeners.signal};window.addEventListener('blur',()=>this.stop(),options);window.addEventListener('pagehide',()=>this.stop(),options);document.addEventListener('visibilitychange',()=>{if(document.hidden)this.stop();},options);
  }
  get available(){return this.#available;}
  async initialize({signal}={}){try{const status=await this.#account.status({signal});this.#available=status.account_management_available===true;}catch{checkAbort(signal);this.#available=false;}return this.#available;}
  assertIdle(){requireValue(!this.#closed&&!this.#starting&&!this.#current?.alive,'busy','Stop synchronization and wait for its current work to finish before changing accounts.');}
  snapshot(){return this.#current?structuredClone(this.#current.snapshot):null;}
  async status({signal}={}){const account=await this.#account.status({signal}),consent=await syncConsentStatus(this.#store,this.#profile,{signal});return{...account,...consent,consent_enabled:consent.enabled,enabled:consent.enabled&&account.connected&&this.#available&&account.account_management_available,encrypted_sync_available:this.#available&&account.account_management_available,job:this.snapshot(),run_protocol:1,pending_export_protocol:account.local_export_available?1:null,pending_export_active:Boolean(this.#exportTask),pending_export_stop_requested:this.#exportController?.signal.aborted??false};}
  cancelExport(){const active=Boolean(this.#exportTask);this.#exportController?.abort();return{stop_requested:active};}
  async exportPending(deliver,{signal}={}){
    checkAbort(signal);this.assertIdle();this.#account.assertIdle();requireValue(typeof deliver==='function'&&!document.hidden&&document.hasFocus(),'cancelled','Export a pending backup from the foreground Game Hub.');this.#starting=true;
    const controller=new AbortController(),abort=()=>controller.abort();this.#exportController=controller;signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
    const task=(async()=>{try{const signal=controller.signal,context=await loadLocalExportContext(this.#store,this.#profile,{signal}),status=await this.#credentials.status({signal});requireValue(!this.#closed&&status.connected&&status.crypto_available&&status.generation===context.generation,'unsupported_capability','Local backup export needs this connected credential generation and its approved crypto artifact.');return await withSyncLease(this.#profile,{signal},async lease=>{try{const unlocked=await this.#credentials.unlock({signal:lease.signal});requireValue(unlocked.generation===context.generation&&unlocked.account.account_id===context.account_id&&unlocked.account.device_id===context.device_id,'permission_expired','The backup account changed before export.');return await exportRetainedUpload({store:this.#store,snapshot:()=>WebSyncJournal.pendingExportSnapshot(this.#store,lease,context),credentials:this.#credentials,lease,context,deliver});}finally{this.#credentials.lock();}});}finally{signal?.removeEventListener('abort',abort);this.#starting=false;if(this.#exportController===controller)this.#exportController=null;}})();this.#exportTask=task;try{return await task;}finally{if(this.#exportTask===task)this.#exportTask=null;}
  }
  async #policy(signal){checkAbort(signal);requireValue(this.#available&&!this.#closed,'unsupported_capability','Encrypted browser synchronization has no admitted product capability.');const status=await this.#credentials.status({signal}),policy=await readSyncNetworkPolicy({signal});requireValue(status.crypto_available&&policy.enabled,'unsupported_capability','Encrypted browser synchronization needs its approved crypto and service policy.');return{status,policy};}
  async configure(input,{signal}={}){
    requireValue(input&&typeof input==='object'&&!Array.isArray(input)&&Object.keys(input).every(key=>['enabled','generation','expected_epoch'].includes(key))&&typeof input.enabled==='boolean','invalid_request','The synchronization consent command is invalid.');
    if(input.enabled){requireValue(!document.hidden&&document.hasFocus(),'cancelled','Enable synchronization from the foreground Game Hub.');const{status,policy}=await this.#policy(signal);requireValue(status.connected&&status.account.registered&&policy.service_audiences.includes(status.account.audience),'permission_denied','Connect a registered account on an enabled service first.');}
    const result=await configureSyncConsent(this.#store,this.#profile,input,value=>validateSettings(value??defaultSettings()),{signal,available:this.#available});if(!result.enabled)this.stop();return result;
  }
  start({signal}={}){
    try{checkAbort(signal);this.assertIdle();this.#account.assertIdle();requireValue(!document.hidden&&document.hasFocus(),'cancelled','Keep Game Hub in the foreground while synchronizing.');}catch(error){return Promise.reject(error);}
    this.#starting=true;const controller=new AbortController(),abort=()=>controller.abort();this.#startController=controller;
    let resolve,reject;const task=new Promise((done,failed)=>{resolve=done;reject=failed;});this.#startTask=task;
    signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
    const finish=()=>{signal?.removeEventListener('abort',abort);this.#starting=false;if(this.#startController===controller)this.#startController=null;if(this.#startTask===task)this.#startTask=null;};
    this.#startOwned(controller).then(value=>{finish();resolve(value);},error=>{finish();reject(error);});return task;
  }
  async #startOwned(controller){
    const signal=controller.signal;
    try{
      const context=await loadSyncContext(this.#store,this.#profile,{signal}),{status,policy}=await this.#policy(signal);requireValue(status.connected&&status.generation===context.generation&&status.account.registered&&policy.service_audiences.includes(status.account.audience),'permission_denied','Enable synchronization for a registered account on this service first.');await this.#store.transaction(['config'],'readonly',tx=>assertSyncContext(tx,context),signal);this.#account.assertIdle();checkAbort(signal);requireValue(!this.#closed&&!document.hidden&&document.hasFocus(),'cancelled','The synchronization opportunity left the foreground.');
      const current={snapshot:{id:crypto.randomUUID(),state:'running',started_at_ms:Date.now(),finished_at_ms:null,stop_requested:false,summary:emptySummary(),error:null},controller,alive:true,http:null,journal:null,timedOut:false,timer:0,task:null};this.#current=current;
      current.timer=setTimeout(()=>{current.timedOut=true;this.#stop(current);},RUN_TIME);current.task=this.#perform(current,policy,context);return structuredClone(current.snapshot);
    }catch(error){controller.abort();throw error;}
  }
  async #perform(current,policy,context){
    let error=null,result=null;
    try{
      result=await withSyncLease(this.#profile,{signal:current.controller.signal},async lease=>{
        try{
          const guard=async(signal=lease.signal)=>{checkAbort(signal);await this.#store.transaction(['config'],'readonly',tx=>assertSyncContext(tx,context),signal);checkAbort(signal);};await guard();
          const unlocked=await this.#credentials.unlock({signal:lease.signal});requireValue(unlocked.generation===context.generation&&unlocked.account.account_id===context.account_id&&unlocked.account.device_id===context.device_id,'permission_expired','The synchronization device changed before work started.');await guard();
          const http=new WebSyncHttp(this.#credentials,policy,context.generation,unlocked.account,{guard});current.http=http;const journal=await WebSyncJournal.open(this.#store,lease,context);current.journal=journal;
          const transport=new WebSyncResumable({credentials:this.#credentials,http,journal,saves:this.#saves,lease,guard,onProgress:transfer=>{if(current.alive)current.snapshot.summary.transfer=transfer;}});
          const reconciler=new WebSyncReconciler({store:this.#store,saves:this.#saves,journal,transport,credentials:this.#credentials,http,lease,context,recovery:this.#recovery,onProgress:summary=>{const previous=current.snapshot.summary,completed=summary.uploaded!==previous.uploaded||summary.downloaded!==previous.downloaded||summary.suppressed!==previous.suppressed||summary.cloud_recovered!==previous.cloud_recovered||summary.cloud_recovery_intact!==previous.cloud_recovery_intact;current.snapshot.summary={...summary,transfer:completed?null:previous.transfer};}});
          const summary=await reconciler.run();await guard();return summary;
        }finally{current.http?.close();current.journal?.close();this.#credentials.lock();}
      });
    }catch(caught){error=failure(caught,current);}
    finally{clearTimeout(current.timer);current.http=null;current.journal=null;current.alive=false;current.snapshot.finished_at_ms=Date.now();}
    if(!error&&current.controller.signal.aborted)error=failure(null,current);if(error){current.snapshot.state=error.code==='cancelled'?'cancelled':'failed';current.snapshot.error=error;}else{current.snapshot.state='complete';current.snapshot.summary=result;}
  }
  #job(id){requireValue(typeof id==='string'&&UUID.test(id)&&this.#current?.snapshot.id===id,'not_found','The synchronization run was not found.');return this.#current;}
  jobStatus(id){return structuredClone(this.#job(id).snapshot);}
  #stop(current){if(!current?.alive)return;current.snapshot.stop_requested=true;current.controller.abort();current.http?.close();current.journal?.close();this.#credentials.lock();}
  stop(){this.#startController?.abort();this.#stop(this.#current);this.#exportController?.abort();}
  cancel(id){const current=this.#job(id);this.#stop(current);return structuredClone(current.snapshot);}
  close(){if(this.#closePromise)return this.#closePromise;let resolve;this.#closePromise=new Promise(done=>{resolve=done;});this.#closed=true;this.#listeners.abort();this.stop();Promise.allSettled([this.#startTask]).then(()=>Promise.allSettled([this.#current?.task,this.#exportTask])).then(()=>resolve());return this.#closePromise;}
}
