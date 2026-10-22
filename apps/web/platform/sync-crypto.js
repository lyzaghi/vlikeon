import{WebPlatformError,checkAbort,integer,requireValue,utf8}from'./errors.js';
import{syncCryptoConfiguration,SYNC_CRYPTO_JSON,SYNC_CRYPTO_BINARY}from'./sync-wasm-policy.js';

export async function readSyncCryptoConfiguration({signal}={}){
  checkAbort(signal);const response=await fetch(new URL('./sync-crypto.json',import.meta.url),{credentials:'omit',redirect:'error',signal});requireValue(response.ok&&response.body,'untrusted_artifact','Browser crypto configuration is unavailable.');const reader=response.body.getReader();let length=0,raw='';const decoder=new TextDecoder('utf-8',{fatal:true});
  try{while(true){const part=await reader.read();if(part.done)break;length+=part.value.length;requireValue(length<=8192,'resource_limit','Browser crypto configuration exceeds its bound.');raw+=decoder.decode(part.value,{stream:true});}raw+=decoder.decode();}finally{await reader.cancel().catch(()=>{});}checkAbort(signal);let value;try{value=JSON.parse(raw);}catch{throw new WebPlatformError('untrusted_artifact','Browser crypto configuration is invalid.');}syncCryptoConfiguration(value);return value;
}
export class WebSyncCrypto{
  #worker;#pending=null;#closed=false;#abort=new AbortController();
  constructor(){
    requireValue(isSecureContext&&!document.hidden&&document.hasFocus()&&typeof Worker==='function'&&typeof crypto?.getRandomValues==='function'&&crypto.subtle,'unsupported_capability','Secure foreground browser crypto workers are unavailable.');
    this.#worker=new Worker(new URL('./sync-crypto-worker.js',import.meta.url),{type:'module',name:'omniplay-sync-crypto'});this.#worker.onmessage=event=>this.#receive(event.data);this.#worker.onerror=()=>this.close('runtime_crashed');this.#worker.onmessageerror=()=>this.close('runtime_crashed');
    const options={signal:this.#abort.signal};window.addEventListener('pagehide',()=>this.close(),options);window.addEventListener('blur',()=>this.close(),options);document.addEventListener('visibilitychange',()=>{if(document.hidden)this.close();},options);
  }
  static async open({signal}={}){
    const config=await readSyncCryptoConfiguration({signal});requireValue(syncCryptoConfiguration(config),'unsupported_capability','Encrypted browser synchronization has no admitted crypto artifact.');checkAbort(signal);const client=new WebSyncCrypto();try{await client.#call({method:'initialize',config},{signal,timeout:30000});return client;}catch(error){client.close();throw error;}
  }
  get closed(){return this.#closed;}
  async execute(command,binary=new ArrayBuffer(0),{signal,timeout=60000}={}){
    checkAbort(signal);let encoded;try{encoded=JSON.stringify(command);}catch{throw new WebPlatformError('invalid_request','The crypto command could not be encoded.');}requireValue(typeof encoded==='string'&&utf8(encoded).length<=SYNC_CRYPTO_JSON&&binary instanceof ArrayBuffer&&binary.byteLength<=SYNC_CRYPTO_BINARY,'resource_limit','The crypto command exceeds its finite buffer budget.');
    // Ownership of binary is transferred; callers must provide a private snapshot.
    const message=await this.#call({method:'execute',command_json:encoded,binary},{signal,timeout,transfer:[binary]});let value;try{value=JSON.parse(message.result_json);}catch{this.close('untrusted_artifact');throw new WebPlatformError('untrusted_artifact','The browser crypto result is invalid.');}
    if(message.code===1){requireValue(typeof value.code==='string'&&/^[a-z_]{1,64}$/.test(value.code)&&typeof value.message==='string'&&value.message.length<=512,'untrusted_artifact','The crypto rejection is invalid.');throw new WebPlatformError(value.code,value.message);}return{value,binary:message.binary};
  }
  #call(value,{signal,timeout=60000,transfer=[]}={}){
    checkAbort(signal);requireValue(!this.#closed&&!this.#pending,'busy','The browser crypto worker is closed or already processing a command.');requireValue(integer(timeout,1,120000),'invalid_request','The browser crypto deadline is invalid.');const id=crypto.randomUUID();
    return new Promise((resolve,reject)=>{const cancel=()=>this.close('cancelled'),timer=setTimeout(()=>this.close('deadline_exceeded'),timeout);this.#pending={id,method:value.method,resolve,reject,timer,signal,cancel};signal?.addEventListener('abort',cancel,{once:true});try{this.#worker.postMessage({id,...value},transfer);}catch{this.close('runtime_crashed');}});
  }
  #receive(message){
    const pending=this.#pending;if(this.#closed||!pending)return;
    if(message?.id!==pending.id){this.close('untrusted_artifact');return;}
    if(message.event==='error'){this.close(typeof message.code==='string'&&/^[a-z_]{1,64}$/.test(message.code)?message.code:'runtime_crashed');return;}
    const ready=pending.method==='initialize'&&message.event==='ready'&&message.protocol===1,result=pending.method==='execute'&&message.event==='result'&&[0,1].includes(message.code)&&typeof message.result_json==='string'&&utf8(message.result_json).length<=SYNC_CRYPTO_JSON&&message.binary instanceof ArrayBuffer&&message.binary.byteLength<=SYNC_CRYPTO_BINARY;
    if(!ready&&!result){this.close('untrusted_artifact');return;}this.#pending=null;clearTimeout(pending.timer);pending.signal?.removeEventListener('abort',pending.cancel);pending.resolve(message);
  }
  close(code='cancelled'){
    if(this.#closed)return;this.#closed=true;this.#abort.abort();this.#worker.terminate();const pending=this.#pending;this.#pending=null;if(pending){clearTimeout(pending.timer);pending.signal?.removeEventListener('abort',pending.cancel);pending.reject(new WebPlatformError(code,code==='deadline_exceeded'?'The browser crypto operation exceeded its deadline.':'The browser crypto session ended. Unlock synchronization again to continue.'));}
  }
}
