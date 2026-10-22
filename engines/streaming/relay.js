import{audience,sign,DOMAINS,StreamError,requireValue,bytes}from'./crypto.js';
async function responseJson(response){
  requireValue(response.body,'network_unavailable','The signaling relay returned no response.');const reader=response.body.getReader(),parts=[];let count=0;
  try{for(;;){const{done,value}=await reader.read();if(done)break;count+=value.length;requireValue(count<=1024*1024,'resource_limit','Signaling response exceeds its bound.');parts.push(value);}}
  catch(error){await reader.cancel().catch(()=>{});throw error;}
  const raw=new Uint8Array(count);let offset=0;for(const part of parts){raw.set(part,offset);offset+=part.length;}let value;try{value=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(raw));}catch{throw new StreamError('network_unavailable','The signaling relay returned an invalid response.');}
  if(!response.ok){const allowed=['unauthorized','resource_limit','not_found','conflict','invalid_request','storage_failure'];throw new StreamError(allowed.includes(value.code)?value.code:'network_unavailable','The signaling request was rejected or is unavailable.');}return value;
}
export class RelayClient{
  #identity;#audience;#allowed;
  constructor(identity,relay,allowed){this.#identity=identity;this.#audience=audience(relay);this.#allowed=allowed;requireValue(Array.isArray(allowed)&&allowed.includes(relay),'permission_denied','This signaling origin is not enabled in the installed product configuration.');}
  get audience(){return this.#audience;}
  async request(operation,{signal}={}){
    requireValue(!signal?.aborted,'cancelled','Signaling request was cancelled.');const body={schema_version:1,audience:this.#audience,issued_at_ms:Date.now(),nonce:crypto.randomUUID(),operation},document=await sign(this.#identity,DOMAINS.request,body),raw=JSON.stringify({public_key:this.#identity.public_key,document});requireValue(bytes(raw).length<=192*1024,'resource_limit','Signaling request exceeds its bound.');
    const controller=new AbortController(),abort=()=>controller.abort(),timer=setTimeout(abort,15000);signal?.addEventListener('abort',abort,{once:true});if(signal?.aborted)abort();
    try{return await responseJson(await fetch(this.#audience+'/v1/relay',{method:'POST',headers:{'Content-Type':'application/json'},body:raw,credentials:'omit',cache:'no-store',redirect:'error',referrerPolicy:'no-referrer',signal:controller.signal}));}
    catch(error){if(error instanceof StreamError)throw error;throw new StreamError(signal?.aborted?'cancelled':'network_unavailable',signal?.aborted?'Signaling request was cancelled.':'The configured signaling relay could not be reached.');}
    finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
  }
}
