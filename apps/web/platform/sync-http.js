import{WebPlatformError,checkAbort,requireValue,integer,UUID,HASH,utf8}from'./errors.js';
import{exact,syncNetworkPolicy,syncAudience,blobHeader,sameHeader,uploadProgress}from'./sync-network-policy.js';
import{Sha256}from'./sha256.js';
import{SYNC_CRYPTO_BINARY}from'./sync-wasm-policy.js';

const CONTROL=256*1024,CHUNK=256*1024,MAX_TIME=8640000000000000;
function base64(bytes){let raw='';for(const byte of bytes)raw+=String.fromCharCode(byte);return btoa(raw).replaceAll('+','-').replaceAll('/','_').replace(/=+$/,'');}
function signedHeader(value){requireValue(exact(value,['certificate','request'])&&['certificate','request'].every(key=>exact(value[key],['payload_base64','signature_base64'])),'corrupt_data','The signed synchronization request is invalid.');const bytes=utf8(JSON.stringify(value));requireValue(bytes.length<=24576,'resource_limit','The synchronization authorization exceeds its header budget.');const encoded=base64(bytes);requireValue(encoded.length<=32768,'resource_limit','The synchronization authorization exceeds its encoded budget.');return encoded;}
function objectHeader(raw){
  requireValue(typeof raw==='string'&&raw.length<=4096&&/^[A-Za-z0-9_-]+$/.test(raw),'corrupt_data','The encrypted object response has no valid header.');let bytes,value;try{bytes=Uint8Array.from(atob(raw.replaceAll('-','+').replaceAll('_','/')),letter=>letter.charCodeAt(0));requireValue(base64(bytes)===raw,'corrupt_data','Encrypted object header encoding is not canonical.');value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));}catch(error){if(error instanceof WebPlatformError)throw error;throw new WebPlatformError('corrupt_data','The encrypted object response header is invalid.');}return value;
}
async function boundedBody(response,limit){
  requireValue(response.body,'corrupt_data','The synchronization service returned an empty response.');const reader=response.body.getReader(),parts=[];let size=0;
  try{while(true){const part=await reader.read();if(part.done)break;size+=part.value.byteLength;requireValue(size<=limit,'resource_limit','The synchronization response exceeds its finite budget.');parts.push(part.value);}}finally{void reader.cancel().catch(()=>{});}const bytes=new Uint8Array(size);let offset=0;for(const part of parts){bytes.set(part,offset);offset+=part.length;}return bytes;
}
function rejected(status){return new WebPlatformError(status===401||status===403?'permission_denied':status===404?'not_found':status===409?'conflict':status===413||status===429?'resource_limit':'network_unavailable','The encrypted save service rejected this operation. Local progress remains available.');}

export class WebSyncHttp{
  #credentials;#policy;#generation;#account;#guard;#pending=new Set();#closed=false;#abort=new AbortController();
  constructor(credentials,policy,generation,account,{guard=null}={}){
    requireValue(guard===null||typeof guard==='function','invalid_request','The synchronization transport guard is invalid.');this.#guard=guard;
    this.#policy=syncNetworkPolicy(policy);requireValue(this.#policy.enabled&&typeof generation==='string'&&UUID.test(generation)&&account&&typeof account.account_id==='string'&&HASH.test(account.account_id)&&this.#policy.service_audiences.includes(syncAudience(account.audience)),'permission_denied','This synchronization service is not enabled by the product policy.');this.#credentials=credentials;this.#generation=generation;this.#account=structuredClone(account);
    const options={signal:this.#abort.signal};window.addEventListener('pagehide',()=>this.close(),options);window.addEventListener('blur',()=>this.close(),options);document.addEventListener('visibilitychange',()=>{if(document.hidden)this.close();},options);
  }
  async #identity(signal){checkAbort(signal);requireValue(!this.#closed&&!document.hidden&&document.hasFocus(),'cancelled','Keep the Game Hub in the foreground for synchronization.');if(this.#guard)await this.#guard(signal);const account=await this.#credentials.assertGeneration(this.#generation,{signal});requireValue(['account_id','device_id','root_public_key','audience'].every(key=>account[key]===this.#account[key]),'permission_expired','The connected synchronization identity changed.');checkAbort(signal);return account;}
  async #fetch(path,headers,body,{signal,timeout=30000,binary=false,limit=CONTROL,verify=()=>{}}={}){
    checkAbort(signal);requireValue(!this.#closed&&this.#pending.size<2&&integer(timeout,1,30000),'busy','The browser synchronization transport is closed, busy or has an invalid deadline.');const controller=new AbortController();let timedOut=false;const cancel=()=>controller.abort(),timer=setTimeout(()=>{timedOut=true;controller.abort();},timeout);this.#pending.add(controller);signal?.addEventListener('abort',cancel,{once:true});const url=this.#account.audience+path;
    try{
      await this.#identity(controller.signal);checkAbort(controller.signal);const response=await fetch(url,{method:'POST',mode:'cors',credentials:'omit',cache:'no-store',redirect:'error',referrerPolicy:'no-referrer',headers,body,signal:controller.signal});requireValue(response.url===url&&['basic','cors'].includes(response.type),'network_unavailable','The encrypted save service redirected or changed its response boundary.');
      if(!response.ok){void response.body?.cancel().catch(()=>{});throw rejected(response.status);}
      const mime=response.headers.get('content-type')?.split(';')[0].trim().toLowerCase();requireValue(mime===(binary?'application/octet-stream':'application/json'),'corrupt_data','The synchronization response has an unexpected content type.');
      const bytes=await boundedBody(response,limit);checkAbort(controller.signal);await this.#identity(controller.signal);verify(response,bytes);
      if(binary)return bytes.buffer;
      try{return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));}catch{throw new WebPlatformError('corrupt_data','The synchronization service returned invalid JSON.');}
    }catch(error){if(timedOut)throw new WebPlatformError('deadline_exceeded','The encrypted save request exceeded its deadline.');if(signal?.aborted||this.#closed)throw new WebPlatformError('cancelled','The encrypted save request was cancelled.');if(error instanceof WebPlatformError)throw error;throw new WebPlatformError('network_unavailable','The encrypted save service is unavailable. Check the connection and retry.');}
    finally{clearTimeout(timer);signal?.removeEventListener('abort',cancel);this.#pending.delete(controller);controller.abort();}
  }
  async #request(operation,{signal,body,verify,binary=false,limit=CONTROL}={}){
    await this.#identity(signal);const signed=await this.#credentials.sign(operation,{signal}),headers={'x-omniplay-authorization':signedHeader(signed)};if(body!==undefined)headers['content-type']='application/octet-stream';return this.#fetch('/v1/sync',headers,body,{signal,verify,binary,limit});
  }
  async root(operation,recoveryKey,{signal,registrationToken=null}={}){
    await this.#identity(signal);const request=await this.#credentials.root(operation,recoveryKey,{signal}),body=JSON.stringify(request);requireValue(utf8(body).length<=32768,'resource_limit','The root synchronization command exceeds its size limit.');const headers={'content-type':'application/json'};
    if(registrationToken!==null){requireValue(typeof registrationToken==='string'&&/^[\x21-\x7e]{32,256}$/.test(registrationToken),'invalid_request','The service registration code must contain 32–256 visible ASCII characters.');headers['x-omniplay-registration']=registrationToken;}
    return this.#fetch('/v1/account',headers,body,{signal});
  }
  async list(after=0,limit=100,{signal}={}){
    requireValue(integer(after,0,Number.MAX_SAFE_INTEGER)&&integer(limit,1,200),'invalid_request','The remote save page range is invalid.');const result=await this.#request({method:'list',params:{after_sequence:after,limit}},{signal});requireValue(Array.isArray(result)&&result.length<=limit,'corrupt_data','The remote save page exceeds its requested size.');let sequence=after;const ids=new Set();
    for(const item of result){requireValue(exact(item,['object_id','sequence','encrypted_bytes','uploaded_at_ms'])&&typeof item.object_id==='string'&&UUID.test(item.object_id)&&!ids.has(item.object_id)&&integer(item.sequence,sequence+1,Number.MAX_SAFE_INTEGER)&&integer(item.encrypted_bytes,16,SYNC_CRYPTO_BINARY)&&integer(item.uploaded_at_ms,0,MAX_TIME),'corrupt_data','The remote save page is not a bounded increasing sequence.');ids.add(item.object_id);sequence=item.sequence;}return result;
  }
  async devices({signal}={}){
    const result=await this.#request({method:'devices'},{signal});requireValue(Array.isArray(result)&&result.length<=128,'corrupt_data','The remote device list exceeds its size limit.');const ids=new Set();for(const item of result){requireValue(exact(item,['device_id','first_seen_at_ms','last_seen_at_ms','revoked_at_ms'])&&typeof item.device_id==='string'&&UUID.test(item.device_id)&&!ids.has(item.device_id)&&integer(item.first_seen_at_ms,0,MAX_TIME)&&integer(item.last_seen_at_ms,0,MAX_TIME)&&(item.revoked_at_ms===null||integer(item.revoked_at_ms,1,MAX_TIME)),'corrupt_data','The remote device record is invalid.');ids.add(item.device_id);}return result;
  }
  async capabilities({signal}={}){
    const result=await this.#request({method:'transfer_capabilities'},{signal});requireValue(exact(result,['protocol','chunk_bytes','max_object_bytes','expiry_ms'])&&result.protocol===1&&result.chunk_bytes===CHUNK&&integer(result.max_object_bytes,SYNC_CRYPTO_BINARY,Number.MAX_SAFE_INTEGER)&&integer(result.expiry_ms,1,86400000),'unsupported_protocol','The save service does not support the required resumable transport.');return result;
  }
  async header(object,{signal}={}){requireValue(typeof object==='string'&&UUID.test(object),'invalid_request','The remote object identity is invalid.');return blobHeader(await this.#request({method:'object_header',params:{object_id:object}},{signal}),this.#account.account_id,object);}
  async chunk(expected,offset,{signal}={}){
    blobHeader(expected,this.#account.account_id);requireValue(integer(offset,0,expected.ciphertext_bytes-1)&&offset%CHUNK===0,'invalid_request','The encrypted chunk offset is invalid.');const length=Math.min(CHUNK,expected.ciphertext_bytes-offset);
    return this.#request({method:'object_chunk',params:{object_id:expected.binding.object_id,offset,bytes:length}},{signal,binary:true,limit:length,verify:(response,bytes)=>{requireValue(bytes.length===length,'corrupt_data','The encrypted object chunk is truncated.');const actual=blobHeader(objectHeader(response.headers.get('x-omniplay-object')),this.#account.account_id,expected.binding.object_id);requireValue(sameHeader(actual,expected)&&response.headers.get('x-omniplay-chunk-sha256')===new Sha256().update(bytes).hex(),'corrupt_data','The encrypted chunk header or transport hash changed.');}});
  }
  async begin(transfer,header,{signal}={}){requireValue(typeof transfer==='string'&&UUID.test(transfer),'invalid_request','The upload identity is invalid.');blobHeader(header,this.#account.account_id);return uploadProgress(await this.#request({method:'upload_begin',params:{transfer_id:transfer,header}},{signal}),transfer,header);}
  async putChunk(transfer,header,offset,blob,{signal}={}){
    blobHeader(header,this.#account.account_id);requireValue(typeof transfer==='string'&&UUID.test(transfer)&&integer(offset,0,header.ciphertext_bytes-1)&&offset%CHUNK===0&&blob instanceof Blob&&blob.size===Math.min(CHUNK,header.ciphertext_bytes-offset),'invalid_request','The encrypted upload chunk is invalid.');checkAbort(signal);const bytes=new Uint8Array(await blob.arrayBuffer());checkAbort(signal);const digest=new Sha256().update(bytes).hex();
    const result=uploadProgress(await this.#request({method:'upload_chunk',params:{transfer_id:transfer,offset,bytes:bytes.length,sha256:digest}},{signal,body:bytes}),transfer,header);requireValue(!result.stored&&result.received_bytes>=offset+bytes.length,'corrupt_data','The service did not acknowledge the submitted encrypted chunk.');return result;
  }
  async commit(transfer,header,{signal}={}){requireValue(typeof transfer==='string'&&UUID.test(transfer),'invalid_request','The upload identity is invalid.');blobHeader(header,this.#account.account_id);const result=uploadProgress(await this.#request({method:'upload_commit',params:{transfer_id:transfer,header}},{signal}),transfer,header);requireValue(result.stored,'corrupt_data','The save service did not confirm immutable publication.');return result;}
  async abort(transfer,{signal}={}){requireValue(typeof transfer==='string'&&UUID.test(transfer),'invalid_request','The upload identity is invalid.');const result=await this.#request({method:'upload_abort',params:{transfer_id:transfer}},{signal});requireValue(exact(result,['aborted'])&&result.aborted===true,'corrupt_data','The save service did not confirm upload cancellation.');return result;}
  close(){if(this.#closed)return;this.#closed=true;this.#abort.abort();for(const controller of this.#pending)controller.abort();}
}
