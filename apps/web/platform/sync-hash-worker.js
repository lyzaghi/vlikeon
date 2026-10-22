import{Sha256}from'./sha256.js';
import{integer,UUID}from'./errors.js';
import{SYNC_CRYPTO_BINARY}from'./sync-wasm-policy.js';

let hash=null,length=0,busy=false;
const valid=(condition)=>{if(!condition)throw new Error('Invalid encrypted hash state');};
onmessage=async event=>{
  const value=event.data,id=value?.id;if(busy||typeof id!=='string'||!UUID.test(id)){postMessage({id:null,event:'error'});close();return;}busy=true;
  try{
    let chunkHash=null;
    if(value.method==='reset'){
      valid(Array.isArray(value.parts)&&value.parts.length<=1025&&integer(value.expected_bytes,0,SYNC_CRYPTO_BINARY));const parts=value.parts;let count=0;for(const part of parts){valid(part instanceof Blob&&integer(part.size,1,262144));count+=part.size;}valid(count===value.expected_bytes);
      hash=new Sha256();length=0;for(const part of parts){const bytes=new Uint8Array(await part.arrayBuffer());valid(bytes.length===part.size);hash.update(bytes);length+=bytes.length;bytes.fill(0);}
    }else{
      valid(value.method==='append'&&hash&&value.offset===length&&value.bytes instanceof ArrayBuffer&&integer(value.bytes.byteLength,1,262144)&&length+value.bytes.byteLength<=SYNC_CRYPTO_BINARY);const bytes=new Uint8Array(value.bytes);chunkHash=new Sha256().update(bytes).hex();hash.update(bytes);length+=bytes.length;bytes.fill(0);
    }
    postMessage({id,event:'complete',bytes:length,sha256:hash.clone().hex(),chunk_sha256:chunkHash});
  }catch{postMessage({id,event:'error'});hash=null;close();}
  finally{busy=false;}
};
