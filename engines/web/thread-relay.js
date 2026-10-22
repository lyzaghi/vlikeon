import{validateThreadPolicy,threadWorkerUrl,threadsAvailable}from'./thread-policy.js';
import{threadPayload}from'./thread-messages.js';

export function blockNestedWorkers(){
  globalThis.Worker=class{constructor(){throw new Error('This emulator has no admitted nested-worker capability.');}};
}

// The UI owns every actual pthread Worker. It can terminate them independently
// when the emulator actor blocks, fails or exceeds its existing request deadline.
export class UiThreadRelay{
  #policy;#url;#parent;#workers=new Map();#closed=false;#last=0;#failed;#pagehide;
  constructor(policy,moduleUrl,parent,onFailure){
    this.#policy=validateThreadPolicy(policy);if(!this.#policy||!threadsAvailable())throw new Error('This browser does not expose isolated emulator threads.');
    this.#url=threadWorkerUrl(this.#policy,moduleUrl);this.#parent=parent;this.#failed=onFailure;
    this.#pagehide=()=>this.#fail();globalThis.addEventListener('pagehide',this.#pagehide,{once:true});
  }
  get workerUrl(){return this.#url;}
  handle(value){
    if(this.#closed)return;
    try{
      if(!value||!Number.isSafeInteger(value.id)||value.id<1||value.id>this.#policy.maximum_workers)throw new Error();
      if(value.kind==='create'){
        if(Object.keys(value).length!==4||value.id<=this.#last||this.#workers.size>=this.#policy.maximum_workers||value.url!==this.#url||
            !value.options||Object.keys(value.options).some(key=>!['type','name','credentials'].includes(key))||value.options.type!=='module'||
            typeof value.options.name!=='string'||value.options.name.length>128||value.options.credentials!=='same-origin')throw new Error();
        this.#last=value.id;
        const worker=new Worker(this.#url,value.options);this.#workers.set(value.id,worker);
        worker.onmessage=event=>{
          if(this.#closed||this.#workers.get(value.id)!==worker)return;
          try{if(event.ports.length)throw new Error();const transfer=threadPayload(event.data,this.#policy.maximum_memory_bytes);this.#parent.postMessage({method:'pthread_message_v1',params:{thread_id:value.id,payload:event.data}},transfer);}
          catch{this.#fail();}
        };
        worker.onerror=event=>{event.preventDefault();this.#fail();};worker.onmessageerror=()=>this.#fail();
      }else if(value.kind==='post'){
        if(Object.keys(value).length!==3)throw new Error();const worker=this.#workers.get(value.id);if(!worker)throw new Error();
        const transfer=threadPayload(value.payload,this.#policy.maximum_memory_bytes);worker.postMessage(value.payload,transfer);
      }else if(value.kind==='terminate'){
        if(Object.keys(value).length!==2)throw new Error();const worker=this.#workers.get(value.id);if(worker){this.#workers.delete(value.id);worker.onmessage=worker.onerror=worker.onmessageerror=null;worker.terminate();}
      }else throw new Error();
    }catch{this.#fail();}
  }
  #fail(){if(this.#closed)return;this.close();this.#failed('The isolated emulator thread pool stopped.');}
  close(){if(this.#closed)return;this.#closed=true;globalThis.removeEventListener('pagehide',this.#pagehide);for(const worker of this.#workers.values()){worker.onmessage=worker.onerror=worker.onmessageerror=null;worker.terminate();}this.#workers.clear();this.#parent=null;}
}

// Emscripten sees the normal asynchronous Worker surface while the actual
// handles stay in the UI. SDK messages bypass the guest-command serialization.
export class RemoteThreadPool{
  #policy;#url;#workers=new Map();#ids=new WeakMap();#next=0;#closed=false;#replacement;#post;#failed;
  constructor(policy,url,post,onFailure){
    this.#policy=validateThreadPolicy(policy);if(!this.#policy||!threadsAvailable())throw new Error('Isolated emulator threads are unavailable.');
    this.#url=url;this.#post=post;this.#failed=onFailure;
    const pool=this;
    this.#replacement=class extends EventTarget{
      onmessage=null;onerror=null;onmessageerror=null;
      constructor(resource,options={}){
        super();if(pool.#closed||pool.#workers.size>=pool.#policy.maximum_workers||pool.#next>=pool.#policy.maximum_workers||new URL(resource,location.href).href!==pool.#url||
            Object.keys(options).some(key=>!['type','name','credentials'].includes(key))||options.type!=='module'||options.credentials!==undefined&&options.credentials!=='same-origin'||
            options.name!==undefined&&(typeof options.name!=='string'||options.name.length>128))throw new Error('The emulator thread request is outside its capability.');
        const id=++pool.#next;pool.#ids.set(this,id);pool.#workers.set(id,this);
        pool.#post({kind:'create',id,url:pool.#url,options:{type:'module',name:options.name??'OmniPlay emulator thread',credentials:'same-origin'}});
      }
      postMessage(payload,options=[]){
        const id=pool.#ids.get(this);if(pool.#closed||pool.#workers.get(id)!==this)throw new Error('The emulator thread is retired.');
        const allowed=threadPayload(payload,pool.#policy.maximum_memory_bytes),transfer=Array.isArray(options)?options:options?.transfer;
        if(!Array.isArray(transfer)||transfer.length>8||new Set(transfer).size!==transfer.length||transfer.some(value=>!allowed.includes(value)))throw new Error('The emulator thread transfer is invalid.');
        // Forward all ordinary buffers; shared memory/module handles retain
        // their structured-clone semantics and are never put in a transfer list.
        pool.#post({kind:'post',id,payload},transfer);
      }
      terminate(){const id=pool.#ids.get(this);if(pool.#workers.get(id)!==this)return;pool.#workers.delete(id);this.onmessage=this.onerror=this.onmessageerror=null;if(!pool.#closed)pool.#post({kind:'terminate',id});}
    };
    globalThis.Worker=this.#replacement;
    if(globalThis.Worker!==this.#replacement)throw new Error('The emulator thread factory could not be installed.');
  }
  receive(params){
    if(this.#closed)return;
    try{
      if(!params||Object.keys(params).length!==2||!Number.isSafeInteger(params.thread_id))throw new Error();
      const worker=this.#workers.get(params.thread_id);if(!worker)return;threadPayload(params.payload,this.#policy.maximum_memory_bytes);
      const event=new MessageEvent('message',{data:params.payload});worker.onmessage?.call(worker,event);worker.dispatchEvent(event);
    }catch{this.#failed();}
  }
  close(){
    if(this.#closed)return;this.#closed=true;
    for(const[id,worker]of [...this.#workers]){worker.terminate();try{this.#post({kind:'terminate',id});}catch{}}
    this.#workers.clear();
    // Keep the closed factory installed until the actor is terminated. A late
    // SDK task must never fall back to constructing an untracked nested Worker.
  }
}
