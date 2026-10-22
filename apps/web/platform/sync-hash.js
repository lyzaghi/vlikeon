import{WebPlatformError,checkAbort,requireValue,integer,HASH}from'./errors.js';
import{SYNC_CRYPTO_BINARY}from'./sync-wasm-policy.js';

export class SyncHash{
  #worker;#pending=null;#closed=false;#signal;#cancel;
  constructor(signal){checkAbort(signal);this.#signal=signal;this.#cancel=()=>this.close('cancelled');this.#worker=new Worker(new URL('./sync-hash-worker.js',import.meta.url),{type:'module',name:'omniplay-encrypted-prefix-hash'});signal?.addEventListener('abort',this.#cancel,{once:true});this.#worker.onerror=()=>this.close('corrupt_data');this.#worker.onmessageerror=()=>this.close('corrupt_data');this.#worker.onmessage=event=>{
    const value=event.data,pending=this.#pending;if(this.#closed||!pending)return;
    if(value?.id!==pending.id||value.event!=='complete'||value.bytes!==pending.bytes||typeof value.sha256!=='string'||!HASH.test(value.sha256)||(pending.append?typeof value.chunk_sha256!=='string'||!HASH.test(value.chunk_sha256):value.chunk_sha256!==null)){this.close('corrupt_data');return;}
    this.#pending=null;clearTimeout(pending.timer);pending.resolve(value);
  };}
  get closed(){return this.#closed;}
  reset(parts,bytes){requireValue(Array.isArray(parts)&&parts.length<=1025&&parts.every(part=>part instanceof Blob&&integer(part.size,1,262144))&&integer(bytes,0,SYNC_CRYPTO_BINARY),'invalid_request','The encrypted prefix hash snapshot is invalid.');return this.#request({method:'reset',parts,expected_bytes:bytes},bytes,false,60000);}
  append(offset,buffer){requireValue(integer(offset,0,SYNC_CRYPTO_BINARY)&&buffer instanceof ArrayBuffer&&integer(buffer.byteLength,1,262144)&&offset+buffer.byteLength<=SYNC_CRYPTO_BINARY,'invalid_request','The encrypted prefix hash chunk is invalid.');return this.#request({method:'append',offset,bytes:buffer},offset+buffer.byteLength,true,10000,[buffer]);}
  #request(value,bytes,append,timeout,transfer=[]){checkAbort(this.#signal);requireValue(!this.#closed&&!this.#pending,'busy','The encrypted prefix hash worker is closed or busy.');return new Promise((resolve,reject)=>{const id=crypto.randomUUID(),timer=setTimeout(()=>this.close('deadline_exceeded'),timeout);this.#pending={id,resolve,reject,bytes,append,timer};try{this.#worker.postMessage({id,...value},transfer);}catch{this.close('corrupt_data');}});}
  close(code='cancelled'){if(this.#closed)return;this.#closed=true;this.#signal?.removeEventListener('abort',this.#cancel);this.#worker.terminate();const pending=this.#pending;this.#pending=null;if(pending){clearTimeout(pending.timer);pending.reject(new WebPlatformError(code,'The encrypted transfer hash operation ended. Retained transfer bytes can be checked again.'));}}
}
