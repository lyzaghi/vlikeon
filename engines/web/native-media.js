import{normalizeMedia,mediaIdentity}from'./media-manifest.js';
import{mountMedia}from'./media-filesystem.js';

export class NativeMedia {
  #header;#bytes;#set;#files=new Map();#cache=new Map();#sequence=0;#unmount=null;#closed=false;
  media=null;id=null;
  static async create(description,shared){
    if(globalThis.crossOriginIsolated!==true||typeof SharedArrayBuffer!=='function'||!(shared instanceof SharedArrayBuffer)||shared.byteLength!==65536+16||typeof Atomics.wait!=='function')throw new Error('Synchronous native media requires isolated shared memory.');
    const media=normalizeMedia(description.media,description.system,description.total);if(await mediaIdentity(media)!==description.content_hash||description.read_limit!==65536||typeof description.set_id!=='string'||!/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(description.set_id))throw new Error('Native media identity or protocol is invalid.');
    const value=new NativeMedia();value.media=media;value.id=description.set_id;value.#set=description.set_id;value.#header=new Int32Array(shared,0,4);value.#bytes=new Uint8Array(shared,16);
    if(Atomics.load(value.#header,0)!==0)throw new Error('The native media read channel was already used.');
    media.assets.forEach((entry,index)=>value.#files.set(entry.name,{entry,handle:{read:(output,{at})=>value.#read(index,output,at)}}));return value;
  }
  #read(index,output,offset){
    if(this.#closed||Atomics.load(this.#header,0)===4||!ArrayBuffer.isView(output)||output.BYTES_PER_ELEMENT!==1||!Number.isSafeInteger(offset)||offset<0)throw new Error('Native media read is closed or invalid.');
    const length=this.media.assets[index].byte_length;if(offset>=length||!output.byteLength)return 0;
    const blockOffset=Math.floor(offset/65536)*65536,key=`${index}:${blockOffset}`;let bytes=this.#cache.get(key);
    if(bytes){this.#cache.delete(key);this.#cache.set(key,bytes);}else{
      if(++this.#sequence>0x7fffffff)throw new Error('The native media sequence limit was reached.');
      if(Atomics.load(this.#header,0)!==0)throw new Error('The native media read channel is no longer ready.');
      const limit=Math.min(65536,length-blockOffset);Atomics.store(this.#header,1,this.#sequence);Atomics.store(this.#header,2,0);Atomics.store(this.#header,0,1);
      self.postMessage({event:'native_media_read',data:{sequence:this.#sequence,set_id:this.#set,asset_index:index,offset:blockOffset,limit}});
      const until=performance.now()+25000;
      while(Atomics.load(this.#header,0)===1){const remaining=until-performance.now();if(remaining<=0){Atomics.compareExchange(this.#header,0,1,4);throw new Error('The native game read timed out.');}Atomics.wait(this.#header,0,1,remaining);}
      if(Atomics.load(this.#header,0)!==2||Atomics.load(this.#header,1)!==this.#sequence||Atomics.load(this.#header,2)!==limit){this.#closed=true;throw new Error('The native game asset could not be read.');}
      bytes=this.#bytes.slice(0,limit);Atomics.store(this.#header,0,0);if(this.#cache.size>=64)this.#cache.delete(this.#cache.keys().next().value);this.#cache.set(key,bytes);
    }
    const relative=offset-blockOffset,count=Math.min(output.byteLength,bytes.length-relative);output.set(bytes.subarray(relative,relative+count));return count;
  }
  mount(core,id){if(this.#closed||id!==this.id||this.#unmount)throw new Error('Native media mount is closed or mismatched.');this.#unmount=mountMedia(core,this.#files);return`/game/${this.media.entry_name}`;}
  readEntry(){const asset=this.media.assets.find(value=>value.name===this.media.entry_name),index=this.media.assets.indexOf(asset);if(this.media.assets.length!==1||asset.byte_length>256*1024*1024)throw new Error('This core requires an unsupported in-memory media size.');const output=new Uint8Array(asset.byte_length);let offset=0;while(offset<output.length)offset+=this.#read(index,output.subarray(offset,Math.min(output.length,offset+65536)),offset);return output;}
  async dispose(){this.#closed=true;Atomics.store(this.#header,0,4);Atomics.notify(this.#header,0);this.#cache.clear();try{this.#unmount?.();}finally{this.#unmount=null;this.#files.clear();}return{cleanup_pending:false};}
}
