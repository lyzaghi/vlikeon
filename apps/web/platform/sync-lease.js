import{checkAbort,requireValue,UUID}from'./errors.js';

const owners=new WeakMap();
export function checkSyncLease(lease,profile){const owner=owners.get(lease);requireValue(owner?.active&&owner.profile===profile,'permission_expired','The browser synchronization operation no longer owns its profile lease.');checkAbort(owner.controller.signal);return owner.controller.signal;}
export async function withSyncLease(profile,{signal}={},operation){
  checkAbort(signal);requireValue(typeof profile==='string'&&UUID.test(profile)&&typeof operation==='function'&&typeof navigator.locks?.request==='function','unsupported_capability','Synchronization requires an exclusive browser profile lock.');
  return navigator.locks.request('omniplay-sync-account-v1:'+profile,{mode:'exclusive',ifAvailable:true},async lock=>{
    requireValue(lock,'busy','Another OmniPlay window owns this synchronization profile.');checkAbort(signal);requireValue(!document.hidden&&document.hasFocus(),'cancelled','Keep the Game Hub in the foreground for synchronization.');
    const controller=new AbortController(),listeners=new AbortController(),cancel=()=>controller.abort(),lease=Object.freeze({profile,signal:controller.signal}),owner={profile,controller,active:true};owners.set(lease,owner);
    signal?.addEventListener('abort',cancel,{once:true});const options={signal:listeners.signal};window.addEventListener('blur',cancel,options);window.addEventListener('pagehide',cancel,options);document.addEventListener('visibilitychange',()=>{if(document.hidden)cancel();},options);
    try{checkSyncLease(lease,profile);return await operation(lease);}
    finally{owner.active=false;controller.abort();listeners.abort();signal?.removeEventListener('abort',cancel);owners.delete(lease);}
  });
}
