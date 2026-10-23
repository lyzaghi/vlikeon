import{checkAbort,requireValue,storageError,UUID,WebPlatformError}from'./errors.js';

const NAME='omniplay-local-v1',VERSION=9;
export function requestValue(request){return new Promise((resolve,reject)=>{request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(request.error);});}

export async function openStore(onVersionChange=()=>{}){
  requireValue(globalThis.isSecureContext&&globalThis.indexedDB,'unsupported_capability','Open OmniPlay over HTTPS in a browser with local storage support.');
  const request=indexedDB.open(NAME,VERSION);let abandoned=false;
  request.onupgradeneeded=event=>{
    if(abandoned){request.transaction.abort();return;}
    const db=request.result;
    if(event.oldVersion<1){
    db.createObjectStore('config',{keyPath:'key'});
    const games=db.createObjectStore('games',{keyPath:'id'});games.createIndex('content_hash','content_hash',{unique:true});games.createIndex('sort',['sort_title','id'],{unique:true});games.createIndex('system_sort',['system','sort_title','id'],{unique:true});
    db.createObjectStore('assets',{keyPath:'hash'});
    const saves=db.createObjectStore('saves',{keyPath:'id'});saves.createIndex('game','game_id');saves.createIndex('slot',['game_id','kind','slot','generation'],{unique:true});saves.createIndex('history',['game_id','generation','created_at_ms','id'],{unique:true});
    const heads=db.createObjectStore('heads',{keyPath:['game_id','kind','slot']});heads.createIndex('game','game_id');
    const runs=db.createObjectStore('performance',{keyPath:'id'});runs.createIndex('game','game_id');runs.createIndex('created','created_at_ms');
    const leases=db.createObjectStore('leases',{keyPath:'id'});leases.createIndex('game','game_id');
    }
    if(event.oldVersion<2){
      const games=request.transaction.objectStore('games');
      games.createIndex('favorite_sort',['favorite','sort_title','id'],{unique:true});
      games.createIndex('system_favorite_sort',['system','favorite','sort_title','id'],{unique:true});
      const records=games.openCursor();records.onsuccess=()=>{const cursor=records.result;if(!cursor)return;cursor.update({...cursor.value,favorite:0});cursor.continue();};
    }
    if(event.oldVersion<3)db.createObjectStore('removed_saves',{keyPath:'id'});
    if(event.oldVersion<4){
      db.createObjectStore('firmware_metadata',{keyPath:'content_hash'});
      db.createObjectStore('firmware_data',{keyPath:'content_hash'});
      const selected=db.createObjectStore('firmware_selections',{keyPath:'key'});selected.createIndex('hash','content_hash');
    }
    if(event.oldVersion<5){
      const origins=db.createObjectStore('save_origins',{keyPath:'save_id'});origins.createIndex('parent','canonical_metadata.parent_id');
      let count=0;const records=request.transaction.objectStore('saves').openCursor();
      records.onsuccess=()=>{const cursor=records.result;if(!cursor)return;
        if(++count>20000||typeof cursor.value?.id!=='string'||!UUID.test(cursor.value.id)){request.transaction.abort();return;}
        origins.add({schema_version:1,save_id:cursor.value.id,kind:'legacy_projection',canonical_metadata:null,canonical_hash:null,imported_from:null,imported_from_hash:null,missing_parent:false});cursor.continue();
      };
    }
    if(event.oldVersion<6){
      const revisions=db.createObjectStore('save_payload_revisions',{keyPath:'save_id'});
      let count=0;const records=request.transaction.objectStore('saves').openCursor();
      records.onsuccess=()=>{const cursor=records.result;if(!cursor)return;
        if(++count>20000||typeof cursor.value?.id!=='string'||!UUID.test(cursor.value.id)){request.transaction.abort();return;}
        revisions.add({save_id:cursor.value.id,revision:crypto.randomUUID()});cursor.continue();
      };
    }
    if(event.oldVersion<7){
      db.createObjectStore('sync_slots',{keyPath:'direction'});
      const parts=db.createObjectStore('sync_parts',{keyPath:['transfer_id','offset']});parts.createIndex('transfer','transfer_id');
      const inbox=db.createObjectStore('sync_inbox',{keyPath:['account_id','object_id']});inbox.createIndex('account','account_id');
      const receipts=db.createObjectStore('sync_receipts',{keyPath:['account_id','object_id']});receipts.createIndex('account','account_id');
    }
    if(event.oldVersion<8)request.transaction.objectStore('saves').createIndex('sync_order',['created_at_ms','generation','id'],{unique:true});
    if(event.oldVersion<9){
      const recovery=db.createObjectStore('sync_recovery',{keyPath:'save_id'});recovery.createIndex('account','account_id');recovery.createIndex('game',['account_id','game_id']);
      for(const name of['sync_slots','sync_inbox']){let count=0;const cursorRequest=request.transaction.objectStore(name).openCursor();cursorRequest.onsuccess=()=>{const cursor=cursorRequest.result;if(!cursor)return;if(++count>(name==='sync_slots'?2:32)||cursor.value?.schema_version!==1||Object.hasOwn(cursor.value,'recovery_request_id')){request.transaction.abort();return;}cursor.update({...cursor.value,recovery_request_id:null});cursor.continue();};}
    }
  };
  const db=await new Promise((resolve,reject)=>{
    request.onerror=()=>reject(storageError(request.error));
    request.onblocked=()=>{abandoned=true;onVersionChange('blocked');reject(new WebPlatformError('busy','Close other OmniPlay tabs before upgrading browser storage.'));};
    request.onsuccess=()=>{if(abandoned)request.result.close();else resolve(request.result);};
  });
  db.onversionchange=()=>{db.close();onVersionChange('closed');};
  db.onclose=()=>onVersionChange('closed');
  const store=new Store(db);
  await store.transaction(['config'],'readwrite',async tx=>{
    const config=tx.store('config');
    const fresh=!(await tx.get('config','profile_id'));
    for(const key of['profile_id','device_id']){const existing=await tx.get('config',key);if(!existing)await tx.request(config.add({key,value:crypto.randomUUID()}));else requireValue(UUID.test(existing.value),'corrupt_data','Local browser identity needs recovery.');}
    const usage=await tx.get('config','usage');
    if(!usage&&fresh)await tx.add('config',{key:'usage',value:{assets_bytes:0,saves_bytes:0,save_count:0,game_count:0}});
    else requireValue(Boolean(usage),'corrupt_data','Browser storage accounting needs recovery.');
  }).catch(error=>{db.close();throw error;});
  return store;
}

export class Store {
  #db;
  constructor(db){this.#db=db;}
  close(){this.#db.close();}
  async transaction(names,mode,body,signal){
    checkAbort(signal);let tx;
    try{tx=this.#db.transaction(names,mode,mode==='readwrite'?{durability:'strict'}:undefined);}catch(error){throw storageError(error);}
    if(mode==='readwrite'&&tx.durability!=='strict'){try{tx.abort();}catch{}throw new WebPlatformError('unsupported_capability','This browser does not expose the required strict storage transaction policy.');}
    const completed=new Promise((resolve,reject)=>{tx.oncomplete=resolve;tx.onabort=()=>reject(tx.error||new DOMException('Transaction aborted','AbortError'));tx.onerror=()=>{};});
    // Attach rejection handling before any awaited request; an IDB abort can
    // arrive before a request promise has unwound through the caller.
    void completed.catch(()=>{});
    const abort=()=>{try{tx.abort();}catch{}};signal?.addEventListener('abort',abort,{once:true});
    const context={store:name=>tx.objectStore(name),request:requestValue,get:(name,key)=>requestValue(tx.objectStore(name).get(key)),all:(name)=>requestValue(tx.objectStore(name).getAll()),put:(name,value)=>requestValue(tx.objectStore(name).put(value)),add:(name,value)=>requestValue(tx.objectStore(name).add(value)),delete:(name,key)=>requestValue(tx.objectStore(name).delete(key))};
    try{
      // Bodies may await IDB requests only. Hashing, file reads, timers and
      // network work must finish before starting this transaction.
      const value=await body(context);await completed;return value;
    }catch(error){abort();await completed.catch(()=>{});checkAbort(signal);throw storageError(error);}
    finally{signal?.removeEventListener('abort',abort);}
  }
  async config(key,fallback=null){const value=await this.transaction(['config'],'readonly',tx=>tx.get('config',key));return value?.value??fallback;}
}
