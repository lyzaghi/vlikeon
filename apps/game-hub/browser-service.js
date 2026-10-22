// Privileged browser operations stay in the component extension worker. No
// tab data is sent to the native broker, sync service or diagnostic exporter.
import {gameHubContext} from './component-context.js';
const INTERNAL=Object.freeze({bookmarks:'chrome://bookmarks/',history:'chrome://history/',downloads:'chrome://downloads/',performance:'chrome://settings/performance',permissions:'chrome://settings/content',privacy:'chrome://settings/privacy',passwords:'chrome://password-manager/passwords',profiles:'chrome://settings/manageProfile',search:'chrome://settings/search'});
const states=new WeakMap();let running=0;
const METHODS=new Set(['browser_info','browser_tabs_page','browser_open_url','browser_search','browser_activate_tab','browser_pin_tab','browser_open_internal','browser_game_guard_acquire','browser_game_guard_release']);
const GUARD_PREFIX='omniplay_game_guard_';
// This journal contains only tab/document IDs and the previous boolean flag. It
// survives worker restarts within this browser session, never browser restart.
const recovery=(async()=>{
  const values=await chrome.storage.session.get(null);
  const currentTabs=await chrome.tabs.query({});
  for(const[key,value]of Object.entries(values)){
    if(!key.startsWith(GUARD_PREFIX))continue;
    if(value&&Number.isSafeInteger(value.tab_id)&&value.tab_id>=0&&typeof value.previous==='boolean'){
      try{const tab=currentTabs.find(tab=>tab.id===value.tab_id);if(tab&&!tab.incognito&&tab.autoDiscardable===false)await chrome.tabs.update(value.tab_id,{autoDiscardable:value.previous});}
      catch{continue;}
    }
    await chrome.storage.session.remove(key);
  }
})().catch(()=>{throw failure('browser_unavailable');});
// Attach a rejection handler immediately; each operation still awaits recovery.
void recovery.catch(()=>{});
function failure(code='browser_unavailable'){return Object.assign(new Error(code),{code});}
function boundedText(value,limit){if(typeof value!=='string')return '';let output='',count=0;for(const symbol of value){if(/[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/.test(symbol))continue;output+=symbol;if(++count>=limit)break;}return output;}
function locationLabel(value){try{const url=new URL(value);if(url.protocol==='https:'||url.protocol==='http:')return boundedText(url.origin,256);if(url.protocol==='chrome:')return 'chrome://'+boundedText(url.hostname,64);if(url.protocol==='chrome-extension:')return url.hostname===chrome.runtime.id?'OmniPlay':'Extension';if(url.protocol==='file:')return 'Local file';}catch{}return 'Browser page';}
function tabInfo(tab){return {id:tab.id,title:boundedText(tab.title,160),origin:locationLabel(tab.url),active:Boolean(tab.active),pinned:Boolean(tab.pinned),audible:Boolean(tab.audible),muted:Boolean(tab.mutedInfo?.muted),discarded:Boolean(tab.discarded),loading:tab.status==='loading'};}
export function isBrowserMethod(method){return typeof method==='string'&&method.startsWith('browser_');}
export function forgetBrowserPage(page){const state=states.get(page);if(state){state.closed=true;state.snapshot=null;clearTimeout(state.snapshotTimer);void state.serial.catch(()=>{}).then(()=>restoreGuard(state)).catch(()=>{});}states.delete(page);}
async function restoreGuard(state){
  const guard=state.guard;if(!guard)return;
  let tab;try{tab=await chrome.tabs.get(guard.tab_id);}catch(error){const tabs=await chrome.tabs.query({});if(tabs.some(tab=>tab.id===guard.tab_id))throw error;await chrome.storage.session.remove(guard.key);state.guard=null;return;}
  if(!tab.incognito&&tab.autoDiscardable===false)await chrome.tabs.update(guard.tab_id,{autoDiscardable:guard.previous});
  await chrome.storage.session.remove(guard.key);state.guard=null;
}
async function acquireGuard(state,ctx,id,check){
  if(typeof id!=='string'||!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(id))throw failure('invalid_request');
  if(state.guard&&state.guard.tab_id!==ctx.tabId)throw failure('tab_unavailable');
  if(state.guard&&!state.guard.armed)await restoreGuard(state);
  if(state.guard){const tab=await targetTab(ctx.tabId,ctx.windowId,check);if(tab.autoDiscardable!==false)throw failure('browser_guard_lost');}
  if(!state.guard){
    const tab=await targetTab(ctx.tabId,ctx.windowId,check);if(typeof tab.autoDiscardable!=='boolean')throw failure();
    const guard={tab_id:tab.id,previous:tab.autoDiscardable,key:GUARD_PREFIX+ctx.documentId,leases:new Set([id]),armed:false};
    const prior=await chrome.storage.session.get(guard.key);check();if(prior[guard.key])throw failure('busy');
    state.guard=guard;
    try{await chrome.storage.session.set({[guard.key]:{tab_id:guard.tab_id,previous:guard.previous}});check();await chrome.tabs.update(tab.id,{autoDiscardable:false});check();guard.armed=true;}catch(error){await restoreGuard(state);throw error;}
  }
  if(!state.guard.leases.has(id)&&state.guard.leases.size>=4)throw failure('busy');state.guard.leases.add(id);return {id,automatic_discard_protection:true};
}
async function context(page,check){
  return gameHubContext(page,check);
}
function webAddress(value){
  if(typeof value!=='string'||value.length>2048||value!==value.trim()||/[\s\u0000-\u0020\u007f-\u009f\u202a-\u202e\u2066-\u2069\\]/u.test(value))throw failure('invalid_address');
  // HTTPS is the default. An explicit HTTP address keeps the browser's normal
  // transport/security warnings; executable, file and internal schemes are denied.
  const candidate=/^[A-Za-z][A-Za-z0-9+.-]*:\/\//.test(value)?value:'https://'+value;
  let url;try{url=new URL(candidate);}catch{throw failure('invalid_address');}
  if(!['https:','http:'].includes(url.protocol)||!url.hostname||url.username||url.password||url.href.length>4096||/^(?:javascript|data|file|chrome|chrome-extension|vbscript|blob|about|ftp|ws|wss):/i.test(value))throw failure('invalid_address');
  return url.href;
}
async function targetTab(id,windowId,check){if(!Number.isSafeInteger(id)||id<0)throw failure('invalid_request');const tab=await chrome.tabs.get(id);check();if(tab.incognito||tab.windowId!==windowId)throw failure('tab_unavailable');return tab;}
async function execute(page,state,message,check){
  if(!METHODS.has(message.command.method))throw failure('unsupported');
  const ctx=await context(page,check),method=message.command.method,params=message.command.params??{};
  if(!params||typeof params!=='object'||Array.isArray(params))throw failure('invalid_request');
  if(method==='browser_info')return {available:true,scope:'current_normal_window',dashboard_persistence:false,game_guard:true,shortcuts:Object.keys(INTERNAL)};
  if(method==='browser_game_guard_acquire')return acquireGuard(state,ctx,params.id,check);
  if(method==='browser_game_guard_release'){
    if(typeof params.id!=='string'||params.id.length!==36)throw failure('invalid_request');
    if(state.guard?.leases.has(params.id)){if(state.guard.leases.size===1)await restoreGuard(state);else state.guard.leases.delete(params.id);}return {released:true};
  }
  if(method==='browser_tabs_page'){
    let offset=0;
    if(params.after==null){
      const all=await chrome.tabs.query({windowId:ctx.windowId});check();const tabs=all.filter(tab=>!tab.incognito&&Number.isSafeInteger(tab.id)).sort((a,b)=>a.index-b.index);
      state.snapshot={id:crypto.randomUUID(),windowId:ctx.windowId,at:Date.now(),tabs:tabs.slice(0,2000).map(tabInfo),total:tabs.length};
      clearTimeout(state.snapshotTimer);state.snapshotTimer=setTimeout(()=>{state.snapshot=null;},60000);
    }else{
      const cursor=params.after;if(typeof cursor!=='string'||cursor.length>64)throw failure('invalid_request');
      const parts=cursor.split(':');if(parts.length!==2||!/^\d{1,4}$/.test(parts[1]))throw failure('invalid_request');offset=Number(parts[1]);
      if(!state.snapshot||state.snapshot.id!==parts[0]||state.snapshot.windowId!==ctx.windowId||Date.now()-state.snapshot.at>60000)throw failure('stale_cursor');
    }
    const snapshot=state.snapshot;if(offset<0||offset>snapshot.tabs.length||offset%40!==0)throw failure('invalid_request');const next=offset+40;
    return {tabs:snapshot.tabs.slice(offset,next),total:snapshot.total,truncated:snapshot.total>2000,next_cursor:next<snapshot.tabs.length?snapshot.id+':'+next:null};
  }
  if(method==='browser_open_url'){const url=webAddress(params.url);check();await chrome.tabs.create({windowId:ctx.windowId,url,active:true,openerTabId:ctx.tabId});return {opened:true};}
  if(method==='browser_open_internal'){if(!Object.hasOwn(INTERNAL,params.page))throw failure('invalid_request');check();await chrome.tabs.create({windowId:ctx.windowId,url:INTERNAL[params.page],active:true,openerTabId:ctx.tabId});return {opened:true};}
  if(method==='browser_search'){
    if(typeof params.text!=='string'||!params.text.trim()||params.text.length>512||/[\u0000-\u001f\u007f]/.test(params.text))throw failure('invalid_request');
    check();const tab=await chrome.tabs.create({windowId:ctx.windowId,url:'about:blank',active:true,openerTabId:ctx.tabId});
    // Target the tab created in this window, rather than the last-focused window.
    // A failed search leaves that blank tab visible; never close user content.
    if(!Number.isSafeInteger(tab.id))throw failure();check();await chrome.search.query({tabId:tab.id,text:params.text.trim()});return {opened:true};
  }
  const tab=await targetTab(params.tab_id,ctx.windowId,check);
  if(method==='browser_activate_tab'){check();await chrome.tabs.update(tab.id,{active:true});return {updated:true};}
  if(typeof params.pinned!=='boolean')throw failure('invalid_request');check();await chrome.tabs.update(tab.id,{pinned:params.pinned});return {updated:true};
}
export async function handleBrowserRequest(page,message){
  let state=states.get(page);if(!state){state={closed:false,inflight:0,seen:new Map(),snapshot:null,serial:Promise.resolve(),guard:null};states.set(page,state);}
  const send=value=>{if(!state.closed)try{page.postMessage({id:message.id,...value});}catch{}};
  const now=Date.now();for(const[id,expiry]of state.seen)if(expiry<now)state.seen.delete(id);
  if(state.seen.has(message.id)){send({status:'error',error:{code:'duplicate_request',message:'Refresh the browser view before trying again.'}});return;}
  if(state.seen.size>=256||state.inflight>=8||running>=32){send({status:'error',error:{code:'busy',message:'Wait before trying another browser operation.',recoverable:true}});return;}
  state.seen.set(message.id,now+120000);state.inflight++;running++;
  const check=()=>{if(state.closed||Date.now()>message.deadline_ms)throw failure('deadline_exceeded');};
  try{await recovery;check();const operation=state.serial.catch(()=>{}).then(()=>{check();return execute(page,state,message,check);});state.serial=operation;const result=await operation;send({status:'ok',result});}
  catch(error){const allowed=new Set(['invalid_address','invalid_request','tab_unavailable','stale_cursor','deadline_exceeded','unsupported']);const code=allowed.has(error?.code)?error.code:'browser_unavailable';send({status:'error',error:{code,message:'The browser operation could not be completed. Refresh the view and try again.',recoverable:true}});}
  finally{state.inflight--;running--;}
}
