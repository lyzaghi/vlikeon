// Readonly Blob-reference inventory. This is neither a checksum audit nor the
// physical disk allocation: browsers may share Blob backing storage and add
// caches, database indexes and filesystem overhead outside these object stores.
import{checkAbort,requireValue,integer}from'./errors.js';

const MIB=1024**2;
const TABLES=Object.freeze([
  {name:'assets',limit:200000,payload:'blob',declared:'bytes',maximum:4*1024**3},
  {name:'saves',limit:20000,payload:'payload',declared:'payload_bytes',maximum:256*MIB},
  {name:'firmware_data',limit:128,payload:'blob',maximum:524288},
  {name:'sync_slots',limit:2,payload:'payload',encrypted:true,maximum:256*MIB+65568},
  {name:'sync_parts',limit:1025,payload:'payload',maximum:262144},
  {name:'sync_inbox',limit:32,payload:'payload',encrypted:true,maximum:256*MIB+65568}
]);
function scan(tx,spec,signal,until){
  return new Promise((resolve,reject)=>{
    const result={store:spec.name,rows:0,blob_count:0,blob_reference_bytes:0,missing_blobs:0,size_mismatches:0,complete:true};
    const request=tx.store(spec.name).openCursor();
    request.onerror=()=>reject(request.error);
    request.onsuccess=()=>{
      try{
        checkAbort(signal);const cursor=request.result;
        if(!cursor){resolve(result);return;}
        // Stop before materializing another record when the explicit bound is
        // exhausted. Partial values are lower bounds, never a healthy zero.
        if(result.rows===spec.limit||performance.now()>=until){result.complete=false;resolve(result);return;}
        result.rows++;const value=cursor.value;
        if(spec.name==='sync_slots'&&value?.direction==='download'&&!Object.hasOwn(value,'payload')){cursor.continue();return;}
        const blob=value?.[spec.payload];
        if(!(blob instanceof Blob))result.missing_blobs++;
        else{
          requireValue(integer(blob.size,0,Number.MAX_SAFE_INTEGER)&&Number.isSafeInteger(result.blob_reference_bytes+blob.size),'resource_limit','Stored binary sizes exceed the inspection range.');
          result.blob_count++;result.blob_reference_bytes+=blob.size;
          const declared=spec.declared?value[spec.declared]:spec.encrypted?value.header?.ciphertext_bytes:null;
          if(blob.size===0||blob.size>spec.maximum||spec.name==='sync_slots'&&value.direction!=='upload'||(spec.declared||spec.encrypted)&&(!Number.isSafeInteger(declared)||declared!==blob.size))result.size_mismatches++;
        }
        cursor.continue();
      }catch(error){reject(error);}
    };
  });
}
function comparison(expected,actual,complete){return{declared:Number.isSafeInteger(expected)&&expected>=0?expected:null,observed:actual,matches:complete&&Number.isSafeInteger(expected)&&expected>=0?expected===actual:null};}
export async function inspectStorage(store,{signal}={}){
  checkAbort(signal);
  return store.transaction(['config','games','firmware_metadata',...TABLES.map(item=>item.name)],'readonly',async tx=>{
    const started=Date.now(),until=performance.now()+15000,usage=(await tx.get('config','usage'))?.value;
    const stores=[];for(const spec of TABLES)stores.push(await scan(tx,spec,signal,until));
    const byName=new Map(stores.map(value=>[value.store,value]));
    const gameCount=await tx.request(tx.store('games').count()),firmwareCount=await tx.request(tx.store('firmware_metadata').count());
    let bytes=0;for(const value of stores){bytes+=value.blob_reference_bytes;requireValue(Number.isSafeInteger(bytes),'resource_limit','Stored binary sizes exceed the inspection range.');}
    const assets=byName.get('assets'),saves=byName.get('saves'),firmware=byName.get('firmware_data');
    checkAbort(signal);
    return{schema_version:1,scope:'indexeddb_blob_references',sample_started_at_ms:started,sampled_at_ms:Date.now(),complete:stores.every(value=>value.complete),blob_reference_bytes:bytes,stores,
      accounting:{assets_bytes:comparison(usage?.assets_bytes,assets.blob_reference_bytes,assets.complete),saves_bytes:comparison(usage?.saves_bytes,saves.blob_reference_bytes,saves.complete),save_count:comparison(usage?.save_count,saves.rows,saves.complete),game_count:comparison(usage?.game_count,gameCount,true),firmware_count:comparison(firmwareCount,firmware.rows,firmware.complete)},
      payload_checksums_verified:false,physical_allocation_measured:false,orphan_ownership_verified:false,changes_made:false};
  },signal);
}
