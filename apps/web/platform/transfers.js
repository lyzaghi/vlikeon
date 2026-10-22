import{MAX_SAVE,MAX_ROM,UUID,HASH,integer,requireValue,fail}from'./errors.js';
import{validateSave}from'./saves.js';
import{Sha256}from'./sha256.js';

const CHUNK=48*1024,TTL=120000;
function encode(bytes){let text='';for(let at=0;at<bytes.length;at+=8192)text+=String.fromCharCode(...bytes.subarray(at,at+8192));return btoa(text);}
function decode(value){
  requireValue(typeof value==='string'&&value.length>0&&value.length<=CHUNK/3*4&&value.length%4===0&&/^[A-Za-z0-9+/]*={0,2}$/.test(value),'invalid_request','Transfer encoding is invalid.');
  let text;try{text=atob(value);}catch{fail('invalid_request','Transfer encoding is invalid.');}
  requireValue(text.length>0&&text.length<=CHUNK&&btoa(text)===value,'invalid_request','Transfer bytes are not canonical.');return Uint8Array.from(text,c=>c.charCodeAt(0));
}
export class WebTransfers {
  #downloads=new Map();#uploads=new Map();#saves;
  constructor(saves){this.#saves=saves;}
  #expire(){const now=performance.now();for(const map of[this.#downloads,this.#uploads])for(const[id,item]of map)if(item.expires<=now){item.controller?.abort();map.delete(id);}}
  close(){for(const item of this.#uploads.values())item.controller.abort();this.#downloads.clear();this.#uploads.clear();}
  open(blob,hash){
    this.#expire();requireValue(blob instanceof Blob&&integer(blob.size,1,MAX_ROM)&&typeof hash==='string'&&HASH.test(hash)&&this.#downloads.size<4,'resource_limit','The content transfer exceeds its bound.');
    const id=crypto.randomUUID();this.#downloads.set(id,{blob,hash,offset:0,hasher:new Sha256(),expires:performance.now()+TTL});return{transfer_id:id,byte_length:blob.size,sha256:hash,chunk_bytes:CHUNK};
  }
  async read(id,offset,limit){
    this.#expire();const item=this.#downloads.get(id);requireValue(item&&!item.reading,'not_found','Transfer is unavailable or expired.');requireValue(integer(offset,0,MAX_ROM)&&offset===item.offset&&integer(limit,1,CHUNK),'invalid_request','Transfer offset or size is invalid.');item.reading=true;
    try{
      const bytes=new Uint8Array(await item.blob.slice(offset,Math.min(item.blob.size,offset+limit)).arrayBuffer());requireValue(bytes.length>0,'corrupt_data','Stored content is truncated.');
      // Cancellation while a Blob read is in flight must not revive its lease.
      requireValue(this.#downloads.get(id)===item,'cancelled','Transfer was cancelled.');item.hasher.update(bytes);item.offset+=bytes.length;item.expires=performance.now()+TTL;const finished=item.offset===item.blob.size;
      if(finished){this.#downloads.delete(id);requireValue(item.hasher.hex()===item.hash,'corrupt_data','Stored content failed its integrity check.');}return{offset,payload_base64:encode(bytes),finished};
    }catch(error){this.#downloads.delete(id);throw error;}finally{item.reading=false;}
  }
  start(input,parent){
    this.#expire();const metadata=validateSave(input);requireValue((parent===null||typeof parent==='string'&&UUID.test(parent))&&metadata.parent_id===parent,'invalid_request','Save parent is invalid.');
    requireValue(this.#uploads.size<2&&metadata.payload_bytes<=MAX_SAVE,'resource_limit','Too many save transfers are active.');const id=crypto.randomUUID();
    this.#uploads.set(id,{metadata,parent,offset:0,parts:[],hasher:new Sha256(),hash:null,blob:null,committing:false,controller:new AbortController(),expires:performance.now()+TTL});return{transfer_id:id,chunk_bytes:CHUNK};
  }
  chunk(id,offset,value){
    this.#expire();const item=this.#uploads.get(id);requireValue(item&&!item.committing&&!item.hash,'not_found','Save transfer is unavailable or expired.');requireValue(integer(offset,0,MAX_SAVE)&&offset===item.offset,'invalid_request','Save transfer offset is invalid.');const bytes=decode(value);
    requireValue(offset+bytes.length<=item.metadata.payload_bytes,'resource_limit','Save transfer exceeds its declared length.');item.hasher.update(bytes);item.parts.push(new Blob([bytes]));item.offset+=bytes.length;item.expires=performance.now()+TTL;return{next_offset:item.offset};
  }
  async commit(id,{signal}={}){
    this.#expire();const item=this.#uploads.get(id);requireValue(item&&!item.committing,'not_found','Save transfer is unavailable or expired.');requireValue(item.offset===item.metadata.payload_bytes,'invalid_state','Save transfer is incomplete.');
    item.hash??=item.hasher.hex();if(item.hash!==item.metadata.payload_hash){this.#uploads.delete(id);fail('corrupt_data','Uploaded save integrity check failed.');}
    item.blob??=new Blob(item.parts,{type:'application/octet-stream'});item.parts=[];item.committing=true;
    const cancel=()=>this.abort(id);signal?.addEventListener('abort',cancel,{once:true});if(signal?.aborted)cancel();
    try{const saved=await this.#saves.commit(item.metadata,item.blob,item.parent,{signal:item.controller.signal});this.#uploads.delete(id);return saved;}
    finally{signal?.removeEventListener('abort',cancel);item.committing=false;item.expires=performance.now()+TTL;}
  }
  abort(id){const item=this.#uploads.get(id);item?.controller.abort();this.#uploads.delete(id);this.#downloads.delete(id);return{aborted:true};}
}
