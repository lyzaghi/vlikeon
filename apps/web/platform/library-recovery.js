import{HASH,MAX_ROM,requireValue,checkAbort,integer}from'./errors.js';
import{Sha256}from'./sha256.js';
import{hashInWorker as hashBlob}from'./hash-client.js';
const key=id=>{requireValue(typeof id==='string'&&/^content:[a-f0-9]{64}$/.test(id),'invalid_request','The recovery content identity is invalid.');return id.slice(8);};
const revision=record=>new Sha256().update(new TextEncoder().encode(`Vlikeon browser retained content v1\0${record.hash}:${record.bytes}`)).hex();
function valid(record){return record&&typeof record.hash==='string'&&HASH.test(record.hash)&&record.blob instanceof Blob&&integer(record.bytes,1,MAX_ROM)&&record.blob.size===record.bytes;}
export class WebLibraryRecovery{
  #store;#library;
  constructor(store,library){this.#store=store;this.#library=library;}
  async list(after=null,{signal}={}){const last=after===null?null:key(after),until=performance.now()+15000;return this.#store.transaction(['assets'],'readonly',async tx=>{
    const candidates=[];let scanned=0,unknown=0;await new Promise((resolve,reject)=>{const request=tx.store('assets').openCursor(last===null?null:IDBKeyRange.lowerBound(last,true));request.onerror=()=>reject(request.error);request.onsuccess=()=>{try{checkAbort(signal);const cursor=request.result;if(!cursor||candidates.length>64){resolve();return;}requireValue(++scanned<=20000&&performance.now()<until,'resource_limit','The retained-content inventory exceeds this recovery group.');const record=cursor.value;if(valid(record)&&cursor.key===record.hash)candidates.push({id:`content:${record.hash}`,revision:revision(record),bytes:record.bytes,interrupted_import:false});else unknown++;cursor.continue();}catch(error){reject(error);}};});const more=candidates.length>64;candidates.length=Math.min(candidates.length,64);return{candidates,next_after:more?candidates.at(-1).id:null,retained_unknown:unknown};
    },signal);}
  async recover(id,expected,title,system,{signal}={}){
    const hash=key(id),record=await this.#store.transaction(['assets'],'readonly',tx=>tx.get('assets',hash),signal);requireValue(valid(record)&&record.hash===hash&&revision(record)===expected,'conflict','Retained content changed. Refresh recovery before retrying.');requireValue(await hashBlob(record.blob,{signal})===hash,'corrupt_data','The retained content failed its actual-byte checksum.');checkAbort(signal);
    const job=this.#library.start(record.blob,title,system);try{while(true){checkAbort(signal);const current=this.#library.status(job.id);if(current.state==='complete')return current.game;if(current.state!=='running')throw Object.assign(new Error(current.error?.message??'Recovery import did not finish.'),{code:current.error?.code??'storage_failure'});await new Promise(resolve=>{const done=()=>{clearTimeout(timer);signal?.removeEventListener('abort',done);resolve();},timer=setTimeout(done,100);signal?.addEventListener('abort',done,{once:true});if(signal?.aborted)done();});}}finally{if(signal?.aborted)this.#library.cancel(job.id);}
  }
}
