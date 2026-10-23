import {isBrowserMethod,handleBrowserRequest,forgetBrowserPage} from './browser-service.js';
import {isGameHubSender,gameHubContext} from './component-context.js';
import {DocumentOwnership,creationMethods} from './component-ownership.js';
const HOST_NAME = 'com.omniplay.broker';
const pages = new Set();
const pending = new Map();
const pageSessions = new Map();
const pageNativeSessions = new Map();
const pagePreparations = new Map();
const pageInputGrants = new Map();
const pageExports = new Map();
const pageMediaJobs=new Map(),pageMediaSets=new Map();
const pageRequests=new Map();
// Native process/window identities stay in this worker, never in Game Hub replies.
const ownedCaptures=new Map();
function retireCapture(id){const value=ownedCaptures.get(id);if(!value)return;ownedCaptures.delete(id);if(value.picker!==null)try{chrome.desktopCapture.cancelChooseDesktopMedia(value.picker);}catch{}void retireNative('stream_capture_release',{capture_id:id,session_id:value.session_id});}
const ownership=new DocumentOwnership();
let nativePort = null;
let authenticating=0;
const pageAuthentication=new WeakMap();
let identityPromise=null;
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
function profileIdentity(){
  if(!identityPromise)identityPromise=(async()=>{
    await chrome.storage.local.setAccessLevel({accessLevel:'TRUSTED_CONTEXTS'});
    const previous=await chrome.storage.local.get('profile_id');
    if(Object.hasOwn(previous,'profile_id')){
      if(typeof previous.profile_id!=='string'||!UUID.test(previous.profile_id)||previous.profile_id==='00000000-0000-0000-0000-000000000000')throw new Error('profile_unavailable');
      return previous.profile_id;
    }
    const profile_id=crypto.randomUUID();
    await chrome.storage.local.set({profile_id});
    return profile_id;
  })().catch(error=>{identityPromise=null;throw error;});
  return identityPromise;
}

function connectNative() {
  if (nativePort) return nativePort;
  const connected=chrome.runtime.connectNative(HOST_NAME);
  nativePort=connected;
  connected.onMessage.addListener(message => {
    if(nativePort!==connected)return;
    if (!message || typeof message.id !== 'string') return;
    const item = pending.get(message.id);
    if (!item) return;
    pending.delete(message.id);
    clearTimeout(item.timer);
    if(item.method==='stream_capture_prepare'&&message.status==='ok'){
      const value=message.result;
      if(!value||!UUID.test(value.capture_id)||!UUID.test(value.game_id)||!UUID.test(value.session_id)||!Number.isSafeInteger(value.process_id)||value.process_id<=0||typeof value.creation_time!=='string'||!/^\d{1,20}$/.test(value.creation_time)||!Number.isSafeInteger(value.expires_at_ms)){connected.disconnect();return;}
      if(!pages.has(item.page)||item.expired){void retireNative('stream_capture_release',{capture_id:value.capture_id,session_id:value.session_id});return;}
      ownedCaptures.set(value.capture_id,{...value,page:item.page,picker:null,choosing:false,window:null});
      const{process_id,creation_time,...publicValue}=value;message={...message,result:publicValue};
    }
    if(item.method==='stream_capture_release'&&message.status==='ok')ownedCaptures.delete(message.result?.capture_id);
    if(message.status==='ok'){
      try{for(const action of ownership.accept(item.page,item.method,item.params,message.result,pages.has(item.page)&&!item.expired))void retireNative(action.method,action.params);}
      catch(error){
        for(const action of error.cleanup||[])void retireNative(action.method,action.params);
        if(!item.expired&&pages.has(item.page))try{item.page.postMessage({id:message.id,status:'error',error:{code:'resource_limit',message:'Reopen Game Hub before creating more local handles.',recoverable:true}});}catch{}
        if(error.message==='invalid_native_handle'){connected.disconnect();}
        return;
      }
    }
    if(item.method==='web_session_begin'&&message.status==='ok'&&typeof message.result?.id==='string'){
      if(pages.has(item.page)&&!item.expired)pageSessions.get(item.page)?.add(message.result.id);
      else void endObservedSession(message.result.id);
    }
    if(item.method==='web_session_heartbeat'&&message.status==='ok'&&['stopped','failed'].includes(message.result?.state))pageSessions.get(item.page)?.delete(message.result.id);
    if(item.method==='session_prepare'&&message.status==='ok'&&typeof message.result?.id==='string'){
      if(pages.has(item.page)&&!item.expired)pageNativeSessions.get(item.page)?.add(message.result.id);else void endNativeSession(message.result.id);
    }
    if(item.method==='session_prepare_start'&&message.status==='ok'&&typeof message.result?.id==='string'){
      if(pages.has(item.page)&&!item.expired)pagePreparations.get(item.page)?.set(message.result.id,null);else void cancelPreparation(message.result.id);
    }
    if(item.method==='session_prepare_status'&&message.status==='ok'&&message.result?.state==='complete'&&typeof message.result?.session?.id==='string'){
      if(pages.has(item.page)&&!item.expired)pagePreparations.get(item.page)?.set(message.result.id,message.result.session.id);
      if(pages.has(item.page)&&!item.expired)pageNativeSessions.get(item.page)?.add(message.result.session.id);
      else if(!pageNativeSessions.get(item.page)?.has(message.result.session.id))void endNativeSession(message.result.session.id);
    }
    if(item.method==='session_prepare_cancel'&&message.status==='ok')pagePreparations.get(item.page)?.delete(message.result?.id);
    if(item.method==='session_prepare_status'&&message.status==='ok'&&['failed','cancelled'].includes(message.result?.state))pagePreparations.get(item.page)?.delete(item.jobId);
    if(['session_stop','session_detach'].includes(item.method)&&message.status==='ok'){
      pageNativeSessions.get(item.page)?.delete(item.sessionId);
      for(const[job,session]of pagePreparations.get(item.page)||[])if(session===item.sessionId)pagePreparations.get(item.page)?.delete(job);
    }
    if(item.method==='stream_input_arm'&&message.status==='ok'&&typeof message.result?.grant_id==='string'&&typeof message.result?.session_id==='string'){
      if(pages.has(item.page)&&!item.expired)pageInputGrants.get(item.page)?.set(message.result.grant_id,message.result.session_id);else void revokeInput(message.result.grant_id,message.result.session_id);
    }
    if(item.method==='stream_input_revoke'&&message.status==='ok')pageInputGrants.get(item.page)?.delete(message.result?.grant_id);
    if(item.method.startsWith('stream_input_')&&message.status==='ok'&&message.result?.state==='revoked')pageInputGrants.get(item.page)?.delete(message.result?.grant_id);
    if(item.method==='save_export_upload_start'&&message.status==='ok'&&typeof message.result?.transfer_id==='string'){
      if(pages.has(item.page)&&!item.expired)pageExports.get(item.page)?.add(message.result.transfer_id);else void abortExport(message.result.transfer_id);
    }
    if(['save_export_upload_commit','transfer_abort'].includes(item.method)&&message.status==='ok')pageExports.get(item.page)?.delete(item.transferId);
    if(item.method==='asset_ranges_start'&&message.status==='ok'&&typeof message.result?.id==='string'){
      if(pages.has(item.page)&&!item.expired)pageMediaJobs.get(item.page)?.add(message.result.id);else void retireMedia('asset_ranges_cancel',{job_id:message.result.id});
    }
    if(item.method==='asset_ranges_collect'&&message.status==='ok'&&typeof message.result?.set_id==='string'){
      if(pages.has(item.page)&&!item.expired)pageMediaSets.get(item.page)?.set(message.result.set_id,item.jobId);else void retireMedia('asset_ranges_close',{set_id:message.result.set_id});
    }
    if(item.method==='asset_ranges_cancel'&&message.status==='ok'){pageMediaJobs.get(item.page)?.delete(item.jobId);for(const[set,job]of pageMediaSets.get(item.page)||[])if(job===item.jobId)pageMediaSets.get(item.page)?.delete(set);}
    if(item.method==='asset_ranges_close'&&message.status==='ok'){const job=pageMediaSets.get(item.page)?.get(item.setId);pageMediaSets.get(item.page)?.delete(item.setId);if(job)pageMediaJobs.get(item.page)?.delete(job);}
    if(item.method==='asset_ranges_status'&&message.status==='ok'&&['failed','cancelled','expired'].includes(message.result?.state))pageMediaJobs.get(item.page)?.delete(item.jobId);
    if(item.method==='session_list'&&message.status==='ok'&&Array.isArray(message.result))message={...message,result:message.result.filter(session=>pageNativeSessions.get(item.page)?.has(session?.id)||pageSessions.get(item.page)?.has(session?.id))};
    if(!item.expired)try { item.page.postMessage(message); } catch { /* The originating page has closed. */ }
  });
  connected.onDisconnect.addListener(() => {
    const error = chrome.runtime.lastError;
    if(nativePort!==connected)return;
    nativePort = null;
    for (const [id, item] of pending) {
      clearTimeout(item.timer);
      try { item.page.postMessage({ id, status: 'error', error: { code: 'runtime_unavailable', message: 'The local library service is disconnected.', recoverable: true } }); } catch {}
    }
    pending.clear();
    for(const sessions of pageSessions.values())sessions.clear();
    for(const sessions of pageNativeSessions.values())sessions.clear();
    for(const preparations of pagePreparations.values())preparations.clear();
    for(const grants of pageInputGrants.values())grants.clear();
    for(const exports of pageExports.values())exports.clear();
    for(const jobs of pageMediaJobs.values())jobs.clear();for(const sets of pageMediaSets.values())sets.clear();
    ownership.reset();
    for(const id of [...ownedCaptures.keys()])retireCapture(id);
    for (const page of pages) { try { page.postMessage({ event: 'host_disconnected', reason: error ? 'host_unavailable' : 'host_closed' }); } catch {} }
  });
  return connected;
}

async function retireNative(method,params,timeout=15000){
  const port=nativePort;if(!port)return;
  try{const profile_id=await profileIdentity();if(nativePort!==port)return;port.postMessage({profile_id,protocol_major:1,protocol_minor:0,id:crypto.randomUUID(),deadline_ms:Date.now()+timeout,command:{method,params}});}catch{}
}
const endObservedSession=session_id=>retireNative('web_session_heartbeat',{session_id,state:'stopped'});
const endNativeSession=session_id=>retireNative('session_detach',{session_id},120000);
const cancelPreparation=job_id=>retireNative('session_prepare_cancel',{job_id},30000);
const revokeInput=(grant_id,session_id)=>retireNative('stream_input_revoke',{grant_id,session_id});
const abortExport=transfer_id=>retireNative('transfer_abort',{transfer_id});
const retireMedia=(method,params)=>retireNative(method,params);

chrome.runtime.onConnect.addListener(page => {
  if(!isGameHubSender(page)||!ownership.open(page)){page.disconnect();return;}
  pages.add(page);
  pageSessions.set(page,new Set());
  pageNativeSessions.set(page,new Set());
  pagePreparations.set(page,new Map());
  pageInputGrants.set(page,new Map());
  pageExports.set(page,new Set());
  pageMediaJobs.set(page,new Set());pageMediaSets.set(page,new Map());
  pageRequests.set(page,new Map());
  page.onMessage.addListener(async message => {
    if (!message || typeof message.id !== 'string' || !UUID.test(message.id) || typeof message.command?.method !== 'string' || message.command.method.length>96 || message.protocol_major !== 1 || message.protocol_minor!==0 || !Number.isSafeInteger(message.deadline_ms)) return;
    const remaining = message.deadline_ms - Date.now();
    if (remaining <= 0 || remaining > 120000) { page.postMessage({ id: message.id, status: 'error', error: { code: 'deadline_exceeded', message: 'Request deadline is invalid.' } }); return; }
    if(isBrowserMethod(message.command.method)){await handleBrowserRequest(page,message);return;}
    const localAuthentication=pageAuthentication.get(page)||0;
    if(localAuthentication>=8||authenticating>=32){try{page.postMessage({id:message.id,status:'error',error:{code:'busy',message:'Wait for the current operation to finish.',recoverable:true}});}catch{}return;}
    pageAuthentication.set(page,localAuthentication+1);authenticating++;
    try{
    let profile_id;
    const live=()=>{if(!pages.has(page)||Date.now()>=message.deadline_ms)throw new Error('page_unavailable');};
    try{await gameHubContext(page,live);profile_id=await profileIdentity();live();await gameHubContext(page,live);}
    catch{if(pages.has(page))try{page.postMessage({id:message.id,status:'error',error:{code:'permission_denied',message:'The Game Hub document or its local profile is unavailable.',recoverable:true}});}catch{}return;}
    const seen=pageRequests.get(page);if(!seen)return;
    for(const[id,expiry]of seen)if(expiry<=Date.now()&&!pending.has(id))seen.delete(id);
    if(seen.has(message.id)){try{page.postMessage({id:message.id,status:'error',error:{code:'duplicate_request',message:'Refresh before retrying this request.',recoverable:true}});}catch{}return;}
    // A replay cache is a memory bound, not a requests-per-two-minutes budget:
    // large transfers and real input streams must not stop after 256 frames.
    // Retain the most recent 1,024 IDs for at most two minutes and never evict
    // an in-flight or late-cleanup correlation.
    while(seen.size>=1024){const oldest=Array.from(seen.keys()).find(id=>!pending.has(id));if(oldest===undefined){try{page.postMessage({id:message.id,status:'error',error:{code:'busy',message:'Wait for the current operation to finish.',recoverable:true}});}catch{}return;}seen.delete(oldest);}
    seen.set(message.id,Date.now()+120000);
    const method=message.command.method,params=message.command.params;
    if((method==='web_session_begin'&&(pageSessions.get(page)?.size||0)>=4)||(['session_prepare','session_prepare_start'].includes(method)&&(pageNativeSessions.get(page)?.size||0)>=4)||(method==='session_prepare_start'&&(pagePreparations.get(page)?.size||0)>=8)||(method==='asset_ranges_start'&&(pageMediaJobs.get(page)?.size||0)>=4)||(method==='stream_input_arm'&&(pageInputGrants.get(page)?.size||0)>=4)){page.postMessage({id:message.id,status:'error',error:{code:'resource_limit',message:'Close an active game or transfer before creating another.',recoverable:true}});return;}
    if(!ownership.permits(page,method,params,{native:pageNativeSessions.get(page),web:pageSessions.get(page),preparations:pagePreparations.get(page),inputs:pageInputGrants.get(page),mediaJobs:pageMediaJobs.get(page),mediaSets:pageMediaSets.get(page)})){page.postMessage({id:message.id,status:'error',error:{code:'permission_denied',message:'This local capability belongs to another Game Hub document or exceeds its limit.'}});return;}
    if (pending.has(message.id)) return;
    if (pending.size >= 32) { page.postMessage({ id: message.id, status: 'error', error: { code: 'busy', message: 'Too many pending requests.', recoverable: true } }); return; }
    const timer = setTimeout(() => {
      const item=pending.get(message.id);
      if(creationMethods.has(item?.method)){item.expired=true;item.timer=setTimeout(()=>pending.delete(message.id),120000);}else pending.delete(message.id);
      try { page.postMessage({ id: message.id, status: 'error', error: { code: 'deadline_exceeded', message: 'The operation took too long.', recoverable: true } }); } catch {}
    }, Math.max(1,message.deadline_ms-Date.now()));
    // Retain only transient handle identities, never account secrets, paths or
    // chunk payloads, while waiting for a native reply or late cleanup.
    const identities={};for(const key of ['grant_id','destination_grant_id','transfer_id','job_id','session_id'])if(typeof params?.[key]==='string')identities[key]=params[key];
    pending.set(message.id, { page, timer, method:message.command.method,params:identities,sessionId:params?.session_id,transferId:params?.transfer_id,jobId:params?.job_id,setId:params?.set_id });
    try {
      live();let command=message.command;
      if(method==='stream_input_arm'){
        const capture=ownedCaptures.get(params?.capture_id);
        if(!capture||capture.page!==page||capture.game_id!==params.game_id||capture.session_id!==params.session_id||capture.pair_id!==params.pair_id||capture.expires_at_ms<=Date.now()||!capture.window)throw new Error('capture_unavailable');
        command={method,params:{...params,capture_window:capture.window}};
      }
      if(method==='stream_capture_release'&&ownedCaptures.get(params?.capture_id)?.page!==page)throw new Error('capture_unavailable');
      connectNative().postMessage({protocol_major:1,protocol_minor:0,id:message.id,deadline_ms:message.deadline_ms,command,profile_id});
    }
    catch { clearTimeout(timer); pending.delete(message.id); page.postMessage({ id: message.id, status: 'error', error: { code: 'runtime_unavailable', message: 'The local library service is unavailable.', recoverable: true } }); }
    }finally{authenticating--;pageAuthentication.set(page,Math.max(0,(pageAuthentication.get(page)||1)-1));}
  });
  page.onDisconnect.addListener(() => {
    forgetBrowserPage(page);
    pages.delete(page);
    for(const[id,capture]of ownedCaptures)if(capture.page===page)retireCapture(id);
    for(const action of ownership.close(page))void retireNative(action.method,action.params);
    for(const session_id of pageSessions.get(page)||[])void endObservedSession(session_id);
    for(const session_id of pageNativeSessions.get(page)||[])void endNativeSession(session_id);
    for(const job_id of pagePreparations.get(page)?.keys()||[])void cancelPreparation(job_id);
    for(const[grant_id,session_id]of pageInputGrants.get(page)||[])void revokeInput(grant_id,session_id);
    for(const transfer_id of pageExports.get(page)||[])void abortExport(transfer_id);
    for(const set_id of pageMediaSets.get(page)?.keys()||[])void retireMedia('asset_ranges_close',{set_id});
    for(const job_id of pageMediaJobs.get(page)||[])void retireMedia('asset_ranges_cancel',{job_id});
    pageSessions.delete(page);
    pageNativeSessions.delete(page);
    pagePreparations.delete(page);
    pageInputGrants.delete(page);
    pageExports.delete(page);
    pageMediaJobs.delete(page);pageMediaSets.delete(page);
    pageRequests.delete(page);
    for (const [id, item] of pending) { if (item.page === page&&!creationMethods.has(item.method)) { clearTimeout(item.timer); pending.delete(id); } }
  });
});

chrome.action.onClicked.addListener(() => chrome.tabs.create({ url: chrome.runtime.getURL('index.html') }));

chrome.runtime.onMessage.addListener((message,sender,respond)=>{
  if(message?.type!=='omniplay_owned_capture_v1')return;
  const page=[...pages].find(page=>page.sender?.documentId===sender.documentId);
  const capture=ownedCaptures.get(message.capture_id);
  if(!page||!isGameHubSender({name:'omniplay-ui',sender})||!capture||capture.page!==page){respond({error:'permission_denied'});return;}
  if(message.action==='cancel'){retireCapture(message.capture_id);respond({released:true});return;}
  if(message.action!=='choose'||typeof message.audio!=='boolean'||capture.choosing||capture.window||capture.expires_at_ms<=Date.now()){respond({error:'invalid_state'});return;}
  capture.choosing=true;
  void(async()=>{
    let timer=0,settled=false;
    const finish=value=>{if(settled)return;settled=true;clearTimeout(timer);capture.choosing=false;capture.picker=null;respond(value);};
    try{
      const check=()=>{if(ownedCaptures.get(message.capture_id)!==capture||!pages.has(page)||capture.expires_at_ms<=Date.now())throw new Error('cancelled');};
      const context=await gameHubContext(page,check);check();
      timer=setTimeout(()=>{retireCapture(message.capture_id);finish({error:'deadline_exceeded'});},45000);
      const picker=chrome.desktopCapture.chooseDesktopMedia(message.audio?['window','audio']:['window'],{id:context.tabId,url:sender.url},{systemAudio:'exclude',windowAudio:message.audio?'window':'exclude',selfBrowserSurface:'exclude',omniplayStrictWindowAudio:message.audio,omniplayProcessId:capture.process_id,omniplayCreationTime:capture.creation_time},(streamId,options)=>{
        const error=chrome.runtime.lastError;
        try{
          check();if(error||!streamId||options?.omniplayOwnedCaptureProtocol!==1||typeof options.omniplayWindowId!=='string'||!/^\d{1,20}$/.test(options.omniplayWindowId)||options.omniplayWindowId==='0'||typeof options.canRequestAudioTrack!=='boolean'||message.audio&&options.canRequestAudioTrack&&options.audioScope!=='application')throw new Error('unsupported_capability');
          capture.window=options.omniplayWindowId;
          finish({stream_id:streamId,audio_present:message.audio&&options.canRequestAudioTrack,owned_capture:{protocol:1,capture_id:capture.capture_id,game_id:capture.game_id,session_id:capture.session_id,expires_at_ms:capture.expires_at_ms}});
        }catch{retireCapture(message.capture_id);finish({error:'capture_refused'});}
      });
      if(!settled)capture.picker=picker;
    }catch{retireCapture(message.capture_id);finish({error:'capture_refused'});}
  })();return true;
});
