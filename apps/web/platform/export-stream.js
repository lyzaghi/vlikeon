import{integer,utf8,requireValue,checkAbort,fail}from'./errors.js';
import{validateSave}from'./saves.js';
import{Sha256}from'./sha256.js';

export class WebExportStreams{
  #files;#active=new Map();#closed=false;
  constructor(files){this.#files=files;}
  #expire(){const now=performance.now();for(const[id,item]of this.#active)if(item.expires<=now)void this.abort(id);}
  async start(input,grantId,signal){
    this.#expire();const metadata=validateSave(input);requireValue(!this.#closed&&!this.#active.size,'busy','Finish the current live export first.');const grant=this.#files.take(grantId,'export'),json=utf8(JSON.stringify(metadata)),prefix=new Uint8Array(12+json.length);new DataView(prefix.buffer).setUint32(8,json.length,true);prefix.set(json,12);
    const id=crypto.randomUUID(),item={metadata,grantId,grant,prefix,writer:null,parts:[],hash:new Sha256(),offset:0,busy:true,expires:performance.now()+120000};this.#active.set(id,item);
    try{
      checkAbort(signal);if(grant.handle){item.writer=await grant.handle.createWritable({keepExistingData:false});checkAbort(signal);requireValue(this.#active.get(id)===item&&!this.#closed,'cancelled','The export owner closed.');await item.writer.write(prefix);}
      checkAbort(signal);requireValue(this.#active.get(id)===item&&!this.#closed,'cancelled','The export owner closed.');item.busy=false;return{transfer_id:id,chunk_bytes:48*1024};
    }catch(error){await this.abort(id);if(item.writer)await item.writer.abort().catch(()=>{});throw error;}
  }
  async chunk(id,offset,encoded,signal){
    this.#expire();const item=this.#active.get(id);requireValue(item&&!item.busy,'permission_expired','The live export is unavailable or expired.');item.busy=true;
    try{
      requireValue(typeof encoded==='string'&&encoded.length>0&&encoded.length<=65536&&encoded.length%4===0&&/^[A-Za-z0-9+/]*={0,2}$/.test(encoded),'invalid_request','Export chunk encoding is invalid.');let binary;try{binary=atob(encoded);}catch{fail('invalid_request','Export chunk encoding is invalid.');}
      requireValue(btoa(binary)===encoded&&binary.length>0&&binary.length<=48*1024&&integer(offset,0,item.metadata.payload_bytes)&&offset===item.offset&&offset+binary.length<=item.metadata.payload_bytes,'invalid_request','Export chunk offset or length is invalid.');const bytes=Uint8Array.from(binary,char=>char.charCodeAt(0));checkAbort(signal);
      if(item.writer)await item.writer.write(bytes);else item.parts.push(new Blob([bytes]));checkAbort(signal);requireValue(this.#active.get(id)===item&&!this.#closed,'cancelled','The export owner closed.');item.hash.update(bytes);item.offset+=bytes.length;item.expires=performance.now()+120000;return{next_offset:item.offset};
    }catch(error){await this.abort(id);throw error;}finally{item.busy=false;}
  }
  async commit(id,grantId,signal){
    this.#expire();const item=this.#active.get(id);requireValue(item&&!item.busy&&item.grantId===grantId,'permission_expired','The live export does not match its destination.');item.busy=true;
    try{
      checkAbort(signal);requireValue(item.offset===item.metadata.payload_bytes&&item.hash.hex()===item.metadata.payload_hash,'content_mismatch','Live export is incomplete or failed its integrity check.');const magic=utf8('OPSV0001');let result;
      if(item.writer){await item.writer.seek(0);checkAbort(signal);await item.writer.write(magic);checkAbort(signal);await item.writer.close();result={exported:true,delivery:'file_handle'};}
      else result=await this.#files.deliver({...item.grant,handle:null},new Blob([magic,item.prefix.subarray(8),...item.parts],{type:'application/octet-stream'}),signal);
      this.#active.delete(id);item.parts=[];return result;
    }catch(error){await this.abort(id);throw error;}
  }
  async abort(id){const item=this.#active.get(id);if(!item)return;this.#active.delete(id);item.parts=[];if(item.writer)await item.writer.abort().catch(()=>{});}
  async close(){this.#closed=true;await Promise.allSettled([...this.#active.keys()].map(id=>this.abort(id)));}
}
