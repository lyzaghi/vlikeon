// User-reviewed, bounded collection of library media without any live owner.
// A preview never grants authority over a changed library: commit rescans in
// the same strict transaction as deletion and accounting adjustment.
import{checkAbort,requireValue,UUID,HASH,integer,utf8}from'./errors.js';
import{Sha256}from'./sha256.js';
import{validateUsage}from'./saves.js';
import{normalizeMedia,mediaExtension}from'../runtime/media-manifest.js';

const STORES=['config','assets','games','saves','leases'],BATCH=64;
const validHash=value=>typeof value==='string'&&HASH.test(value);
const validId=value=>typeof value==='string'&&UUID.test(value)&&value!=='00000000-0000-0000-0000-000000000000';
function frame(hash,value){const raw=utf8(value),length=new Uint8Array(4);new DataView(length.buffer).setUint32(0,raw.length,true);hash.update(length).update(raw);}
function walk(tx,store,range,maximum,visit,{signal,until}){
  return new Promise((resolve,reject)=>{
    let count=0;const request=tx.store(store).openCursor(range);
    request.onerror=()=>reject(request.error);
    request.onsuccess=()=>{try{checkAbort(signal);const cursor=request.result;if(!cursor){resolve(count);return;}requireValue(count<maximum&&performance.now()<until,'resource_limit','The unused-content ownership scan exceeded its bound. No content was removed.');count++;const keepGoing=visit(cursor.value,cursor.key);if(keepGoing===false){resolve(count);return;}cursor.continue();}catch(error){reject(error);}};
  });
}
async function snapshot(tx,after,signal){
  const until=performance.now()+15000,profile=(await tx.get('config','profile_id'))?.value,usage=validateUsage((await tx.get('config','usage'))?.value);
  requireValue(validId(profile),'corrupt_data','The local content owner needs recovery.');
  const hash=new Sha256();frame(hash,'omniplay.unused-assets.v1');frame(hash,profile);frame(hash,after??'');frame(hash,String(usage.assets_bytes));
  const candidates=new Map();let more=false,last=null;
  await walk(tx,'assets',after===null?undefined:IDBKeyRange.lowerBound(after,true),BATCH+1,(value,key)=>{
    if(candidates.size===BATCH){more=true;return false;}
    requireValue(validHash(key)&&value?.hash===key&&Object.keys(value).length===3&&integer(value.bytes,1,4*1024**3)&&value.blob instanceof Blob&&value.blob.size===value.bytes,'corrupt_data','Stored game content needs recovery before cleanup.');
    candidates.set(key,{bytes:value.bytes,referenced:false});last=key;frame(hash,key);frame(hash,String(value.bytes));
  },{signal,until});
  // The complete bounded reference set is mandatory even for a small batch.
  // Malformed/unknown game media never turns into permission to erase a Blob.
  const games=new Set();
  await walk(tx,'games',undefined,50000,value=>{
    requireValue(validId(value?.id)&&validHash(value.content_hash)&&integer(value.byte_length,1,4*1024**3),'corrupt_data','A library media owner needs recovery before cleanup.');
    let assets;
    try{const media=value.media??{schema_version:1,format:'raw_rom',entry_name:`game.${mediaExtension('raw_rom',value.system)}`,assets:[{name:`game.${mediaExtension('raw_rom',value.system)}`,content_hash:value.content_hash,byte_length:value.byte_length}]};assets=normalizeMedia(media,value.system,value.byte_length).assets;}catch{requireValue(false,'corrupt_data','A library media manifest needs recovery before cleanup.');}
    games.add(value.id);frame(hash,value.id);frame(hash,value.content_hash);
    for(const asset of assets){frame(hash,asset.content_hash);frame(hash,String(asset.byte_length));const candidate=candidates.get(asset.content_hash);if(candidate){requireValue(candidate.bytes===asset.byte_length,'corrupt_data','A shared media size differs from its library owner.');candidate.referenced=true;}}
  },{signal,until});
  const saveCount=await walk(tx,'saves',undefined,20000,value=>{
    requireValue(validId(value?.id)&&validId(value.game_id)&&validHash(value.game_content_hash),'corrupt_data','A saved-progress owner needs recovery before cleanup.');
    requireValue(games.has(value.game_id),'conflict','Saved progress has no library owner. Recover or manage it before removing unused content.');
    frame(hash,value.id);frame(hash,value.game_id);frame(hash,value.game_content_hash);
    const candidate=candidates.get(value.game_content_hash);if(candidate)candidate.referenced=true;
  },{signal,until});
  await walk(tx,'leases',undefined,64,value=>{
    requireValue(validId(value?.game_id)&&value.info&&['ready','running','paused','stopped','failed'].includes(value.info.state)&&integer(value.expires_at_ms,0,Number.MAX_SAFE_INTEGER),'corrupt_data','A game lifecycle owner needs recovery before cleanup.');
    requireValue(!['ready','running','paused'].includes(value.info.state)||value.expires_at_ms<=Date.now(),'busy','Close active games before inspecting unused game content.');
  },{signal,until});
  let bytes=0;const unused=[];
  for(const[key,value]of candidates){frame(hash,value.referenced?'retained':'unused');if(!value.referenced){bytes+=value.bytes;unused.push(key);}}
  requireValue(Number.isSafeInteger(bytes)&&usage.assets_bytes>=bytes,'corrupt_data','Game content accounting needs recovery before cleanup.');
  frame(hash,String(games.size));frame(hash,String(saveCount));frame(hash,more?'more':'end');
  return{preview:{schema_version:1,revision:hash.hex(),after_hash:after,scanned_assets:candidates.size,unused_assets:unused.length,unused_bytes:bytes,next_after:more?last:null,more_available:more,library_games:games.size},unused,usage};
}
export class WebUnusedAssets{
  #store;#active=false;
  constructor(store){this.#store=store;}
  async preview(after=null,{signal}={}){
    requireValue(after===null||validHash(after),'invalid_request','The unused-content cursor is invalid.');return this.#perform('readonly',tx=>snapshot(tx,after,signal).then(value=>value.preview),signal);
  }
  async remove(after,revision,{signal}={}){
    requireValue((after===null||validHash(after))&&validHash(revision),'invalid_request','Refresh the unused-content preview before choosing removal.');
    return this.#perform('readwrite',async tx=>{
      const value=await snapshot(tx,after,signal);requireValue(value.preview.revision===revision,'conflict','Library content changed after the preview. Refresh and choose again.');
      checkAbort(signal);for(const key of value.unused)await tx.delete('assets',key);
      if(value.unused.length)await tx.put('config',{key:'usage',value:{...value.usage,assets_bytes:value.usage.assets_bytes-value.preview.unused_bytes}});
      return{removed_assets:value.unused.length,removed_blob_reference_bytes:value.preview.unused_bytes,next_after:value.preview.next_after,more_available:value.preview.more_available};
    },signal);
  }
  async #perform(mode,body,signal){
    checkAbort(signal);requireValue(!this.#active,'busy','An unused-content ownership scan is still finishing.');this.#active=true;
    try{return await this.#store.transaction(STORES,mode,body,signal);}finally{this.#active=false;}
  }
}
