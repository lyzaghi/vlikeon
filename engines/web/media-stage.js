import{Sha256}from'./sha256.js';
import{normalizeMedia,mediaIdentity}from'./media-manifest.js';
import{mountMedia}from'./media-filesystem.js';

const UUID=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
const CHUNK=48*1024,BUCKET='omniplay-runtime-media-v1';
async function acquireLease(){
  if(typeof navigator.locks?.request!=='function')throw new Error('Media staging requires an origin storage lease.');
  let resolveReady,rejectReady,release;const ready=new Promise((resolve,reject)=>{resolveReady=resolve;rejectReady=reject;}),held=new Promise(resolve=>{release=resolve;});
  const completion=navigator.locks.request('omniplay.runtime-media.v1',{mode:'exclusive',ifAvailable:true},async lock=>{if(!lock){resolveReady(false);return;}resolveReady(true);await held;});
  completion.catch(rejectReady);if(!await ready){await completion;throw new Error('Another game is using this origin media staging area.');}
  return async()=>{release();await completion;};
}
async function generatedFiles(directory){
  const files=[];for await(const[name,handle]of directory.entries()){if(files.length>=129||handle.kind!=='file'||!(name==='owner.json'||/^payload-(00[0-9]{2}|01[01][0-9]|012[0-7])$/.test(name)))return null;files.push(name);}return files;
}
async function retireDirectory(bucket,id){
  if(!UUID.test(id))return false;let directory;
  try{directory=await bucket.getDirectoryHandle(id);}catch(error){if(error.name==='NotFoundError')return true;throw error;}
  const files=await generatedFiles(directory);if(!files)return false;
  for(const name of files)await directory.removeEntry(name);await bucket.removeEntry(id);return true;
}
async function collectStale(bucket){
  let inspected=0,retained=0;
  for await(const[name,handle]of bucket.entries()){
    if(++inspected>32)throw new Error('The runtime staging directory needs storage recovery.');
    if(handle.kind!=='directory'||!UUID.test(name)){retained++;continue;}
    try{if(!await retireDirectory(bucket,name))retained++;}catch{retained++;}
  }
  if(retained>=4)throw new Error('Too many stale runtime staging areas could not be reclaimed.');
}

export class MediaStage {
  #bucket=null;#directory=null;#release=null;#files=new Map();#current=null;#next=0;#sealed=false;#unmount=null;#closed=false;
  #created=performance.now();#activity=this.#created;
  id=crypto.randomUUID();media=null;total=0;
  static async create({media,system,total,content_hash}){
    const stage=new MediaStage();
    try{
      stage.media=normalizeMedia(media,system,total);stage.total=total;if(await mediaIdentity(stage.media)!==content_hash)throw new Error('Media staging identity does not match the game.');
      if(typeof navigator.storage?.getDirectory!=='function')throw new Error('This browser cannot stage bounded disc files.');
      stage.#release=await acquireLease();
      const root=await navigator.storage.getDirectory();stage.#bucket=await root.getDirectoryHandle(BUCKET,{create:true});await collectStale(stage.#bucket);
      if(typeof navigator.storage.estimate==='function'){const space=await navigator.storage.estimate();if(Number.isFinite(space.quota)&&Number.isFinite(space.usage)&&space.quota-space.usage<total+64*1024*1024)throw new Error('There is not enough browser storage to stage this game.');}
      stage.#directory=await stage.#bucket.getDirectoryHandle(stage.id,{create:true});
      const marker=await stage.#directory.getFileHandle('owner.json',{create:true});if(typeof marker.createSyncAccessHandle!=='function')throw new Error('File-backed game storage is unavailable in this browser.');const handle=await marker.createSyncAccessHandle();
      try{const bytes=new TextEncoder().encode(JSON.stringify({schema_version:1,purpose:'runtime_media',stage_id:stage.id,asset_count:stage.media.assets.length}));await handle.truncate(0);if(handle.write(bytes,{at:0})!==bytes.length)throw new Error('Runtime staging ownership could not be written.');await handle.flush();}finally{await handle.close();}
      return stage;
    }catch(error){await stage.dispose().catch(()=>{});throw error;}
  }
  #check(id){if(this.#closed||id!==this.id)throw new Error('Media staging capability is closed or mismatched.');if(!this.#sealed&&(performance.now()-this.#activity>120000||performance.now()-this.#created>1800000))throw new Error('Media staging deadline expired.');this.#activity=performance.now();}
  async startAsset({stage_id,name}){
    this.#check(stage_id);const entry=this.media.assets[this.#next];if(this.#sealed||this.#current||!entry||entry.name!==name)throw new Error('Media staging asset order is invalid.');
    const path=`payload-${String(this.#next).padStart(4,'0')}`,file=await this.#directory.getFileHandle(path,{create:true}),handle=await file.createSyncAccessHandle();
    let accepted=false;
    try{await handle.truncate(0);if(handle.getSize()!==0||typeof handle.read!=='function'||typeof handle.write!=='function')throw new Error('Synchronous media storage is unavailable.');this.#current={entry,handle,offset:0,hash:new Sha256()};accepted=true;return{chunk_bytes:CHUNK};}finally{if(!accepted)await handle.close();}
  }
  chunk({stage_id,name,offset,bytes}){
    this.#check(stage_id);const current=this.#current;
    if(this.#sealed||!current||current.entry.name!==name||!Number.isSafeInteger(offset)||offset!==current.offset||!(bytes instanceof ArrayBuffer)||!bytes.byteLength||bytes.byteLength>CHUNK||bytes.byteLength>current.entry.byte_length-offset)throw new Error('Media staging chunk is invalid.');
    const data=new Uint8Array(bytes),written=current.handle.write(data,{at:offset});if(written!==data.length)throw new Error('Browser storage could not completely stage the content.');current.hash.update(data);current.offset+=data.length;return{next_offset:current.offset};
  }
  async finishAsset({stage_id,name}){
    this.#check(stage_id);const current=this.#current;
    if(this.#sealed||!current||current.entry.name!==name||current.offset!==current.entry.byte_length||current.hash.hex()!==current.entry.content_hash)throw new Error('Staged media content failed its integrity check.');
    await current.handle.flush();if(current.handle.getSize()!==current.entry.byte_length)throw new Error('The staged media size changed.');
    this.#files.set(name,{entry:current.entry,handle:current.handle});this.#current=null;this.#next++;return{assets_complete:this.#next};
  }
  seal({stage_id}){this.#check(stage_id);if(this.#current||this.#next!==this.media.assets.length||this.#files.size!==this.media.assets.length)throw new Error('Media staging is incomplete.');this.#sealed=true;return{stage_id:this.id,media_protocol:1};}
  mount(core,id){this.#check(id);if(!this.#sealed||this.#unmount)throw new Error('Media staging is not ready to mount.');this.#unmount=mountMedia(core,this.#files);return`/game/${this.media.entry_name}`;}
  readEntry(){const record=this.#files.get(this.media.entry_name);if(!this.#sealed||!record||record.entry.byte_length>256*1024*1024)throw new Error('This core requires an unsupported in-memory media size.');const result=new Uint8Array(record.entry.byte_length);let offset=0;while(offset<result.length){const part=result.subarray(offset,Math.min(result.length,offset+1024*1024)),count=record.handle.read(part,{at:offset});if(!Number.isSafeInteger(count)||count<=0||count>part.length)throw new Error('The staged entry could not be completely read.');offset+=count;}return result;}
  async dispose(){
    if(this.#closed)return{cleanup_pending:false};this.#closed=true;let pending=false;
    try{this.#unmount?.();}catch{pending=true;}this.#unmount=null;
    const handles=new Set([...this.#files.values()].map(value=>value.handle));if(this.#current)handles.add(this.#current.handle);
    for(const handle of handles)try{await handle.close();}catch{pending=true;}this.#files.clear();this.#current=null;
    if(this.#bucket&&this.#directory)try{if(!await retireDirectory(this.#bucket,this.id))pending=true;}catch{pending=true;}
    try{await this.#release?.();}catch{pending=true;}this.#release=null;this.#directory=null;this.#bucket=null;return{cleanup_pending:pending};
  }
}
