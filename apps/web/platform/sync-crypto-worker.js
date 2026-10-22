import{WebPlatformError,integer,requireValue,UUID,utf8}from'./errors.js';
import{syncCryptoConfiguration,admitSyncWasm,syncWasi,SYNC_CRYPTO_JSON,SYNC_CRYPTO_BINARY}from'./sync-wasm-policy.js';

let instance=null,busy=false,retired=false;
const decoder=new TextDecoder('utf-8',{fatal:true});
const exact=(value,keys)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));
function span(pointer,length,max){pointer>>>=0;length>>>=0;const memory=instance.exports.memory.buffer;requireValue(length<=max&&pointer<=memory.byteLength&&length<=memory.byteLength-pointer,'untrusted_artifact','The crypto ABI returned an invalid owned buffer.');return new Uint8Array(memory,pointer,length);}
function stop(){if(retired)return;retired=true;try{if(instance)new Uint8Array(instance.exports.memory.buffer).fill(0);}catch{}instance=null;close();}
async function initialize(raw){
  const config=syncCryptoConfiguration(raw);requireValue(config,'unsupported_capability','Browser encrypted synchronization has no admitted crypto artifact.');
  const url=new URL('./sync-crypto.wasm',import.meta.url),response=await fetch(url,{credentials:'omit',redirect:'error',cache:'force-cache'});requireValue(response.ok&&response.type!=='opaque'&&response.body,'untrusted_artifact','The browser crypto artifact could not be read.');
  const reader=response.body.getReader(),blocks=[];let length=0;
  try{while(true){const part=await reader.read();if(part.done)break;length+=part.value.length;requireValue(length<=config.wasm_bytes,'untrusted_artifact','The browser crypto artifact exceeds its recorded size.');blocks.push(part.value);}}finally{await reader.cancel().catch(()=>{});}
  requireValue(length===config.wasm_bytes,'untrusted_artifact','The browser crypto artifact is truncated.');const bytes=new Uint8Array(length);let offset=0;for(const block of blocks){bytes.set(block,offset);offset+=block.length;}blocks.length=0;
  const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),value=>value.toString(16).padStart(2,'0')).join('');requireValue(digest===config.wasm_sha256,'untrusted_artifact','The browser crypto artifact changed.');const policy=admitSyncWasm(bytes),module=await WebAssembly.compile(bytes);
  const imports=WebAssembly.Module.imports(module);requireValue(imports.length===policy.imports.length&&imports.every(item=>item.module==='wasi_snapshot_preview1'&&item.kind==='function'&&policy.imports.includes(item.name)),'untrusted_artifact','The crypto engine import view changed.');
  const wasi=syncWasi(()=>{requireValue(instance,'invalid_state','The crypto instance is not ready.');return instance.exports.memory;});instance=await WebAssembly.instantiate(module,{wasi_snapshot_preview1:wasi});bytes.fill(0);
  if(typeof instance.exports._initialize==='function')instance.exports._initialize();requireValue(instance.exports.op_protocol()===1,'unsupported_protocol','The browser crypto ABI is unsupported.');
}
function execute(command,binary){
  requireValue(instance&&typeof command==='string','invalid_state','The browser crypto instance is unavailable.');const source=utf8(command);requireValue(integer(source.length,1,SYNC_CRYPTO_JSON)&&binary instanceof ArrayBuffer&&binary.byteLength<=SYNC_CRYPTO_BINARY,'resource_limit','The browser crypto command exceeds its buffer budget.');
  const api=instance.exports;let control=0,payload=0,guestSafe=true;
  const call=(name,...args)=>{guestSafe=false;const value=api[name](...args);guestSafe=true;return value;};
  try{
    call('op_clear_result');control=call('op_alloc',source.length);requireValue(control!==0,'resource_limit','The crypto control allocation failed.');span(control,source.length,SYNC_CRYPTO_JSON).set(source);
    if(binary.byteLength){payload=call('op_alloc',binary.byteLength);requireValue(payload!==0,'resource_limit','The crypto payload allocation failed.');span(payload,binary.byteLength,SYNC_CRYPTO_BINARY).set(new Uint8Array(binary));}
    source.fill(0);new Uint8Array(binary).fill(0);const code=call('op_command',control,payload);control=0;payload=0;
    requireValue(code===0||code===1,'untrusted_artifact','The crypto command result is invalid.');const resultLength=call('op_result_len'),binaryLength=call('op_binary_len');requireValue(integer(resultLength,1,SYNC_CRYPTO_JSON)&&integer(binaryLength,0,SYNC_CRYPTO_BINARY),'untrusted_artifact','The crypto output exceeds its bound.');
    const resultJson=decoder.decode(span(call('op_result_ptr'),resultLength,SYNC_CRYPTO_JSON));requireValue(resultJson.startsWith('{')&&resultJson.endsWith('}'),'untrusted_artifact','The crypto output is not an object.');
    requireValue(code===0||binaryLength===0,'untrusted_artifact','Rejected crypto output contained binary data.');const output=span(call('op_binary_ptr'),binaryLength,SYNC_CRYPTO_BINARY).slice().buffer;return{code,result_json:resultJson,binary:output};
  }finally{source.fill(0);new Uint8Array(binary).fill(0);if(guestSafe){if(control)call('op_free',control);if(payload)call('op_free',payload);call('op_clear_result');}}
}
onmessage=async event=>{
  const value=event.data,id=value?.id;if(retired)return;
  if(typeof id!=='string'||!UUID.test(id)||busy){postMessage({id:typeof id==='string'&&id.length<=36?id:null,event:'error',code:'busy'});return;}
  busy=true;
  try{
    if(value.method==='initialize'){requireValue(exact(value,['id','method','config'])&&!instance,'invalid_state','The crypto worker is already initialized.');await initialize(value.config);postMessage({id,event:'ready',protocol:1});}
    else{requireValue(value.method==='execute'&&exact(value,['id','method','command_json','binary']),'invalid_request','The browser crypto worker request is invalid.');const result=execute(value.command_json,value.binary);postMessage({id,event:'result',...result},[result.binary]);}
  }catch(error){postMessage({id,event:'error',code:error instanceof WebPlatformError?error.code:'runtime_crashed'});stop();}
  finally{busy=false;}
};
