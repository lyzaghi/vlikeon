import{WebPlatformError,checkAbort,requireValue,UUID}from'./errors.js';
import{exact,syncAudience,readSyncNetworkPolicy}from'./sync-network-policy.js';
import{WebSyncHttp}from'./sync-http.js';
import{withSyncLease}from'./sync-lease.js';

const RESULT_TTL=5*60*1000,ACTION_TIME=110000;
const kinds={create:['kind','audience'],recover:['kind','audience','recovery_key'],register:['kind','recovery_key','registration_token'],renew:['kind','recovery_key'],devices:['kind'],revoke_device:['kind','recovery_key','device_id'],delete_account:['kind','recovery_key'],disconnect:['kind']};
const codes=new Set(['invalid_request','unsupported_capability','unsupported_protocol','permission_denied','permission_expired','not_found','conflict','resource_limit','cancelled','deadline_exceeded','network_unavailable','storage_failure','corrupt_data','untrusted_artifact','busy']);
function failure(error,cancelled=false,timedOut=false){const code=timedOut?'deadline_exceeded':cancelled?'cancelled':codes.has(error?.code)?error.code:'network_unavailable';return{code,message:code==='cancelled'?'The account request was cancelled.':code==='deadline_exceeded'?'The account request exceeded its deadline. Check account state before retrying.':code==='permission_denied'||code==='permission_expired'?'Account authorization failed. Check the recovery key and current device access.':'The account operation could not complete. Local progress remains available.',recoverable:true};}
function validateAction(value){
  requireValue(value&&Object.hasOwn(kinds,value.kind)&&exact(value,kinds[value.kind]),'invalid_request','The browser account action is invalid.');
  if(Object.hasOwn(value,'audience'))syncAudience(value.audience);
  if(Object.hasOwn(value,'recovery_key'))requireValue(typeof value.recovery_key==='string'&&/^OP1-[A-Za-z0-9_-]{43}$/.test(value.recovery_key),'invalid_request','The account recovery key has an invalid format.');
  if(value.kind==='register')requireValue(typeof value.registration_token==='string'&&/^[\x21-\x7e]{32,256}$/.test(value.registration_token),'invalid_request','The registration code must contain 32–256 visible ASCII characters.');
  if(value.kind==='revoke_device')requireValue(typeof value.device_id==='string'&&UUID.test(value.device_id),'invalid_request','The device identity is invalid.');return{...value};
}
function wipe(value){if(value&&typeof value==='object')try{if(Object.hasOwn(value,'recovery_key'))value.recovery_key='';if(Object.hasOwn(value,'registration_token'))value.registration_token='';}catch{}}

export class WebSyncAccount{
  #store;#credentials;#profile;#current=null;#starting=false;#startController=null;#startTask=null;#closePromise=null;#closed=false;#abort=new AbortController();#http=null;#assertSyncIdle;
  constructor(store,credentials,profile,{assertSyncIdle=()=>{}}={}){
    requireValue(typeof profile==='string'&&UUID.test(profile)&&typeof assertSyncIdle==='function','invalid_request','The browser account profile is invalid.');this.#store=store;this.#credentials=credentials;this.#profile=profile;this.#assertSyncIdle=assertSyncIdle;
    const options={signal:this.#abort.signal};window.addEventListener('pagehide',event=>{if(event.persisted)this.#suspend();else this.close();},options);window.addEventListener('blur',()=>this.#suspend(),options);document.addEventListener('visibilitychange',()=>{if(document.hidden)this.#suspend();},options);
  }
  #suspend(){this.#startController?.abort();if(this.#current?.alive){this.#current.snapshot.stop_requested=true;this.#current.controller.abort();}this.#http?.close();this.#credentials.lock();}
  #expire(){const current=this.#current;if(current?.result!==null&&current?.result!==undefined&&(performance.now()>=current.expires||Date.now()>=current.expiresAtMs)){wipe(current.result);current.result=null;clearTimeout(current.expiryTimer);Object.assign(current.snapshot,{state:'expired',result_available:false,result_expired:true});}}
  snapshot(){this.#expire();return this.#current?structuredClone(this.#current.snapshot):null;}
  assertIdle(){this.#expire();requireValue(!this.#closed&&!this.#starting&&!this.#current?.alive&&this.#current?.result==null,'busy','Collect or dismiss the account result before starting synchronization or another account action.');}
  async status({signal}={}){
    const credentials=await this.#credentials.status({signal});let policy=null,networkError=null;try{policy=await readSyncNetworkPolicy({signal});}catch(error){checkAbort(signal);networkError=error.code??'untrusted_artifact';}
    const available=credentials.crypto_available===true&&policy?.enabled===true&&Boolean(navigator.locks?.request)&&globalThis.isSecureContext&&typeof Worker==='function'&&Boolean(globalThis.crypto?.subtle)&&typeof CryptoKey==='function';
    const localExport=credentials.connected&&credentials.account?.registered===true&&credentials.crypto_available===true&&Boolean(navigator.locks?.request)&&globalThis.isSecureContext&&typeof Worker==='function'&&Boolean(globalThis.crypto?.subtle)&&typeof CryptoKey==='function';
    return{connected:credentials.connected,enabled:false,account:credentials.account,policy:'explicit_foreground',credential_store:'origin_bound_webcrypto',action_protocol:1,action:this.snapshot(),job:null,account_management_available:available,local_export_available:localExport,encrypted_sync_available:false,service_audiences:policy?.service_audiences??[],network_error:networkError,crypto_error:credentials.crypto_error};
  }
  start(raw,{signal}={}){
    let action;try{checkAbort(signal);this.assertIdle();this.#assertSyncIdle();requireValue(!document.hidden&&document.hasFocus()&&typeof navigator.locks?.request==='function','unsupported_capability','Account changes require foreground browser origin locks.');action=validateAction(raw);}catch(error){wipe(raw);return Promise.reject(error);}
    wipe(raw);this.#starting=true;const controller=new AbortController(),abort=()=>controller.abort();this.#startController=controller;
    let resolve,reject;const task=new Promise((done,failed)=>{resolve=done;reject=failed;});this.#startTask=task;
    signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
    const finish=()=>{signal?.removeEventListener('abort',abort);this.#starting=false;if(this.#startController===controller)this.#startController=null;if(this.#startTask===task)this.#startTask=null;};
    this.#startOwned(action,controller).then(value=>{finish();resolve(value);},error=>{finish();reject(error);});return task;
  }
  async #startOwned(action,controller){
    const signal=controller.signal;
    try{
      const policy=action.kind==='disconnect'?null:await readSyncNetworkPolicy({signal});
      if(policy){const status=await this.#credentials.status({signal});requireValue(status.crypto_available&&policy.enabled,'unsupported_capability','Browser account access needs the explicitly admitted crypto artifact and service policy.');const audience=Object.hasOwn(action,'audience')?action.audience:status.account?.audience;requireValue(policy.service_audiences.includes(audience),'permission_denied','This account service is not enabled by the product policy.');}
      checkAbort(signal);this.#assertSyncIdle();requireValue(!this.#closed&&!document.hidden&&document.hasFocus(),'cancelled','Browser account access was closed or left the foreground.');if(this.#current)clearTimeout(this.#current.expiryTimer);
      const current={snapshot:{id:crypto.randomUUID(),kind:action.kind,state:'running',started_at_ms:Date.now(),finished_at_ms:null,result_available:false,result_expired:false,stop_requested:false,error:null},controller,alive:true,result:null,expires:0,expiresAtMs:0,expiryTimer:0,deadline:0,timedOut:false};this.#current=current;
      current.deadline=setTimeout(()=>{current.timedOut=true;current.snapshot.stop_requested=true;current.controller.abort();this.#credentials.lock();this.#http?.close();},ACTION_TIME);
      current.task=this.#perform(current,action,policy);return structuredClone(current.snapshot);
    }finally{if(!this.#current?.alive)wipe(action);}
  }
  async #perform(current,action,policy){
    let result=null,error=null;
    try{result=await withSyncLease(this.#profile,{signal:current.controller.signal},lease=>this.#execute(action,policy,lease.signal));}
    catch(caught){error=failure(caught,current.controller.signal.aborted,current.timedOut);}
    finally{wipe(action);this.#http?.close();this.#http=null;this.#credentials.lock();clearTimeout(current.deadline);current.alive=false;current.snapshot.finished_at_ms=Date.now();}
    if(this.#closed){wipe(result);current.result=null;Object.assign(current.snapshot,{state:'discarded',result_available:false});return;}
    if(current.controller.signal.aborted&&!error){wipe(result);result=null;error=failure(null,true,current.timedOut);}
    if(error){current.snapshot.state=error.code==='cancelled'?'cancelled':'failed';current.snapshot.error=error;return;}
    current.result=result;current.expires=performance.now()+RESULT_TTL;current.expiresAtMs=Date.now()+RESULT_TTL;current.snapshot.state='complete';current.snapshot.result_available=true;current.expiryTimer=setTimeout(()=>this.#expire(),RESULT_TTL);
  }
  async #execute(action,policy,signal){
    checkAbort(signal);
    if(action.kind==='disconnect'){const state=await this.#credentials.localStatus({signal});await this.#credentials.disconnect(state.generation,{signal});return{disconnected:true};}
    if(action.kind==='create'){const value=await this.#credentials.provision('create',action.audience,'',{signal});return{account_id:value.account.account_id,recovery_key:value.recovery_key,registered:false,registration_error:null};}
    if(action.kind==='recover'){
      const recovered=await this.#credentials.provision('recover',action.audience,action.recovery_key,{signal,verify:async pending=>{const http=new WebSyncHttp(pending.credentials,policy,pending.generation,pending.account);this.#http=http;try{await http.list(0,1,{signal});}finally{http.close();this.#http=null;}}});return{...recovered.account,profile_id:this.#profile,after_sequence:0};
    }
    const current=await this.#credentials.status({signal});requireValue(current.connected&&policy.service_audiences.includes(current.account.audience),'permission_denied','Connect an enabled synchronization account first.');
    if(action.kind==='renew'){await this.#credentials.renew(action.recovery_key,{signal});return{renewed:true};}
    const http=new WebSyncHttp(this.#credentials,policy,current.generation,current.account);this.#http=http;
    if(action.kind==='register'){
      const value=await http.root({method:'create',params:{root_public_key:current.account.root_public_key}},action.recovery_key,{signal,registrationToken:action.registration_token});requireValue(exact(value,['created','account_id'])&&value.created===true&&value.account_id===current.account.account_id,'corrupt_data','The service registration acknowledgement changed identity.');await this.#credentials.markRegistered(current.generation,{signal});return{registered:true};
    }
    requireValue(current.account.registered,'permission_denied','Complete account registration before this remote account operation.');
    if(action.kind==='devices')return http.devices({signal});
    if(action.kind==='revoke_device'){const value=await http.root({method:'revoke_device',params:{device_id:action.device_id}},action.recovery_key,{signal});requireValue(exact(value,['revoked'])&&value.revoked===true,'corrupt_data','The service did not acknowledge device revocation.');if(action.device_id===current.account.device_id)await this.#credentials.disconnect(current.generation,{signal});return{revoked:true};}
    const value=await http.root({method:'delete_account'},action.recovery_key,{signal});requireValue(exact(value,['deleted'])&&value.deleted===true,'corrupt_data','The service did not acknowledge account deletion.');await this.#credentials.disconnect(current.generation,{signal});return{deleted:true};
  }
  #job(id){requireValue(typeof id==='string'&&UUID.test(id)&&this.#current?.snapshot.id===id,'not_found','The browser account action was not found.');this.#expire();return this.#current;}
  actionStatus(id){return structuredClone(this.#job(id).snapshot);}
  cancel(id){const current=this.#job(id);if(current.alive){current.snapshot.stop_requested=true;current.controller.abort();this.#http?.close();this.#credentials.lock();}return structuredClone(current.snapshot);}
  collect(id){const current=this.#job(id);requireValue(!current.alive,'busy','The account operation is still finishing.');if(current.snapshot.error)throw new WebPlatformError(current.snapshot.error.code,current.snapshot.error.message);requireValue(current.result!==null,'not_found','The account result was already collected, dismissed or expired.');const result=current.result;current.result=null;clearTimeout(current.expiryTimer);Object.assign(current.snapshot,{state:'collected',result_available:false});return result;}
  discard(id){const current=this.#job(id);requireValue(!current.alive,'busy','Cancel and wait for the account action before dismissing its result.');wipe(current.result);current.result=null;clearTimeout(current.expiryTimer);Object.assign(current.snapshot,{state:'discarded',result_available:false});return structuredClone(current.snapshot);}
  close(){if(this.#closePromise)return this.#closePromise;let resolve;this.#closePromise=new Promise(done=>{resolve=done;});this.#closed=true;this.#suspend();this.#abort.abort();if(this.#current){clearTimeout(this.#current.expiryTimer);wipe(this.#current.result);this.#current.result=null;}Promise.allSettled([this.#startTask]).then(()=>Promise.allSettled([this.#current?.task])).then(()=>resolve());return this.#closePromise;}
}
