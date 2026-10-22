import{MAX_ROM,UUID,HASH,integer,utf8,requireValue,checkAbort,fail}from'./errors.js';
import{validateUsage}from'./saves.js';
import{snapshotInWorker}from'./hash-client.js';
import{planDiscBundle}from'./disc-plan.js';
import{normalizeMedia,mediaIdentity}from'../runtime/media-manifest.js';

const EXT={nes:'nes',gb:'gb',gbc:'gbc',snes:'sfc',gba:'gba',mega_drive:'gen',ps1:'bin',n64:'z64',nds:'nds',psp:'iso'};
const FORMATS={iso:['ps1','psp'],chd:['ps1','psp'],pbp:['ps1','psp'],cso:['psp'],n64_z64:['n64'],n64_v64:['n64'],n64_n64:['n64']};
const FORMAT_EXT={iso:'iso',chd:'chd',pbp:'pbp',cso:'cso',n64_z64:'z64',n64_v64:'v64',n64_n64:'n64'};
const normalized=value=>value.normalize('NFKC').toLowerCase();
export function manifest(record){if(!record)fail('not_found','The game was not found.');const{sort_title,last_played_at_ms,playtime_ms,favorite,...value}=record;return value;}
function favoriteFlag(record){requireValue(record,'not_found','The game was not found.');requireValue(record.favorite===0||record.favorite===1,'corrupt_data','Library favorites need recovery.');return record.favorite===1;}
function summary(record){return{favorite:favoriteFlag(record),id:record.id,title:record.title,system:record.system,content_hash:record.content_hash,byte_length:record.byte_length,created_at_ms:record.created_at_ms,last_played_at_ms:record.last_played_at_ms,playtime_ms:record.playtime_ms,content_format:record.media?.format??null,asset_count:record.media?.assets?.length??1};}
function encodeCursor(value){const bytes=utf8(JSON.stringify(value));let text='';for(const byte of bytes)text+=String.fromCharCode(byte);return btoa(text).replaceAll('+','-').replaceAll('/','_').replaceAll('=','');}
function decodeCursor(value){try{requireValue(typeof value==='string'&&value.length<=4096&&/^[A-Za-z0-9_-]+$/.test(value),'invalid_request','Library cursor is invalid.');const bytes=Uint8Array.from(atob(value.replaceAll('-','+').replaceAll('_','/')),c=>c.charCodeAt(0));return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));}catch{fail('invalid_request','Library cursor is invalid.');}}
async function detectFormat(blob,hint,system){
  const data=new Uint8Array(await blob.slice(0,16).arrayBuffer()),text=new TextDecoder('latin1').decode(data);
  let format=text.startsWith('MComprHD')?'chd':text.startsWith('\x00PBP')?'pbp':text.startsWith('CISO')||text.startsWith('ZISO')?'cso':null;
  if(data.length>=4){const key=Array.from(data.subarray(0,4)).join(',');format??=({'128,55,18,64':'n64_z64','55,128,64,18':'n64_v64','64,18,55,128':'n64_n64'})[key]??null;}
  if(!format&&blob.size>=0x8006&&new TextDecoder().decode(await blob.slice(0x8001,0x8006).arrayBuffer())==='CD001')format='iso';
  if(!format&&hint!==null&&hint!==undefined){requireValue(hint==='raw_rom'||FORMATS[hint],'unsupported_capability','This content format needs another import path.');format=hint;}
  format??='raw_rom';requireValue(format==='raw_rom'||FORMATS[format]?.includes(system),'content_mismatch','The selected format does not belong to this system.');
  requireValue(!text.startsWith('PK\x03\x04')&&!text.startsWith('7z\xbc\xaf'),'unsupported_capability','Extract the game locally before importing it.');return format;
}

export class WebLibrary {
  #store;#jobs=new Map();
  constructor(store){this.#store=store;}
  async get(id){requireValue(UUID.test(id),'invalid_request','Game identity is invalid.');return manifest(await this.#store.transaction(['games'],'readonly',tx=>tx.get('games',id)));}
  async page({query=null,system=null,after=null,limit=100,favorites_only=false}={}){
    requireValue((query===null||typeof query==='string'&&utf8(query).length<=512)&&(system===null||Object.hasOwn(EXT,system))&&integer(limit,1,200)&&typeof favorites_only==='boolean','invalid_request','Library query is invalid.');
    const search=normalized(query??''),cursor=after===null?null:decodeCursor(after);
    if(cursor)requireValue(cursor.version===2&&cursor.query===search&&cursor.system===system&&cursor.favorites_only===favorites_only&&Array.isArray(cursor.last)&&cursor.last.length===2&&typeof cursor.last[0]==='string'&&UUID.test(cursor.last[1]),'invalid_request','The cursor belongs to another library query.');
    return this.#store.transaction(['games'],'readonly',async tx=>{
      const totalAll=await tx.request(tx.store('games').count()),index=tx.store('games').index(favorites_only?(system===null?'favorite_sort':'system_favorite_sort'):(system===null?'sort':'system_sort'));
      const prefix=system===null?[]:[system];if(favorites_only)prefix.push(1);const range=prefix.length?IDBKeyRange.bound(prefix,[...prefix,[]]):null;let total=0;const selected=[];
      await new Promise((resolve,reject)=>{const request=index.openCursor(range);request.onerror=()=>reject(request.error);request.onsuccess=()=>{
        const item=request.result;if(!item){resolve();return;}const game=item.value;
        if(game.sort_title.includes(search)){
          ++total;const key=[game.sort_title,game.id];if((!cursor||indexedDB.cmp(key,cursor.last)>0)&&selected.length<=limit)selected.push(summary(game));
        }item.continue();
      };});
      const more=selected.length>limit,games=selected.slice(0,limit),last=games.at(-1),next=more?encodeCursor({version:2,query:search,system,favorites_only,last:[normalized(last.title),last.id]}):null;
      return{games,next_cursor:next,total_matching:total,total_all:totalAll};
    });
  }
  async favorite(id){requireValue(UUID.test(id),'invalid_request','Game identity is invalid.');return this.#store.transaction(['games'],'readonly',async tx=>({game_id:id,favorite:favoriteFlag(await tx.get('games',id))}));}
  async setFavorite(id,value,{signal}={}){
    requireValue(UUID.test(id)&&typeof value==='boolean','invalid_request','Favorite settings are invalid.');
    return this.#store.transaction(['games'],'readwrite',async tx=>{const game=await tx.get('games',id);favoriteFlag(game);await tx.put('games',{...game,favorite:value?1:0});return{game_id:id,favorite:value};},signal);
  }
  #launch(title,system,work){
    requireValue(!Array.from(this.#jobs.values()).some(job=>job.value.state==='running'),'busy','Wait for the current import to finish.');
    requireValue(typeof title==='string'&&title.trim().length>0&&utf8(title).length<=512&&Object.hasOwn(EXT,system),'invalid_request','Game title or system is invalid.');
    for(const[id,job]of this.#jobs)if(job.value.state!=='running')this.#jobs.delete(id);
    const value={id:crypto.randomUUID(),state:'running',progress:{phase:'preparing',copied_bytes:0,total_bytes:0,asset_index:0,asset_count:0},game:null,error:null},controller=new AbortController();
    const job={value,controller,committing:false,expired:false,timer:null};this.#jobs.set(value.id,job);
    job.timer=setTimeout(()=>{if(value.state==='running'&&!job.committing){job.expired=true;controller.abort();}},30*60*1000);
    void Promise.resolve().then(async()=>{
      checkAbort(controller.signal);requireValue(typeof navigator.locks?.request==='function','unsupported_capability','This browser does not expose the required cross-tab import lease. Use a supported browser or the native app.');
      return navigator.locks.request('omniplay-library-import-v1',{mode:'exclusive',ifAvailable:true},async lease=>{requireValue(lease,'busy','Another OmniPlay tab is importing content. Wait for it to finish.');checkAbort(controller.signal);return work(job,title.trim(),system);});
    }).then(game=>{value.state='complete';value.game=game;value.progress.phase='complete';value.progress.copied_bytes=game.byte_length;value.progress.total_bytes=game.byte_length;},error=>{
      value.state=job.expired?'failed':error.code==='cancelled'||error.name==='AbortError'?'cancelled':'failed';value.error={code:job.expired?'deadline_exceeded':error.code??'storage_failure',message:job.expired?'The disc import exceeded its preparation deadline.':error.code?error.message:'The selected content could not be stored.',recoverable:true};
    }).finally(()=>{clearTimeout(job.timer);});
    return structuredClone(value);
  }
  start(blob,title,system,hint=null){
    requireValue(blob instanceof Blob&&integer(blob.size,1,MAX_ROM),'invalid_request','The selected content file length is invalid.');
    return this.#launch(title,system,async(job,name,selectedSystem)=>{
      const signal=job.controller.signal,format=await detectFormat(blob,hint,selectedSystem);checkAbort(signal);
      const plan={format,entry_name:`game.${FORMAT_EXT[format]??EXT[selectedSystem]}`,assets:[{name:`game.${FORMAT_EXT[format]??EXT[selectedSystem]}`,blob}],disc_entries:[],total:blob.size};
      return this.#importPlan(job,name,selectedSystem,plan);
    });
  }
  startBundle(scope,entry,title,system,encoding){
    requireValue(scope?.entries?.includes(entry),'permission_denied','The selected disc entry point is not in this folder capability.');
    return this.#launch(title,system,async(job,name,selectedSystem)=>{
      const plan=await planDiscBundle(scope,entry,selectedSystem,encoding,{signal:job.controller.signal});return this.#importPlan(job,name,selectedSystem,plan);
    });
  }
  async #importPlan(job,title,system,plan){
    const signal=job.controller.signal,prepared=[];let copied=0;job.value.progress={phase:'copying',copied_bytes:0,total_bytes:plan.total,asset_index:0,asset_count:plan.assets.length};
    for(const[index,asset]of plan.assets.entries()){
      checkAbort(signal);job.value.progress.asset_index=index+1;const before=copied;
      const snapshot=await snapshotInWorker(asset.blob,{signal,onProgress:bytes=>{job.value.progress.copied_bytes=before+bytes;}});checkAbort(signal);
      requireValue(snapshot.blob.size===asset.blob.size&&HASH.test(snapshot.sha256),'corrupt_data','The copied content snapshot is incomplete.');
      copied+=snapshot.blob.size;requireValue(copied<=MAX_ROM,'resource_limit','The copied disc set exceeds four GiB.');prepared.push({name:asset.name,content_hash:snapshot.sha256,blob:snapshot.blob});
    }
    requireValue(copied===plan.total,'content_mismatch','Disc content length changed during import.');
    const media=normalizeMedia({schema_version:1,format:plan.format,entry_name:plan.entry_name,assets:prepared.map(asset=>({name:asset.name,content_hash:asset.content_hash,byte_length:asset.blob.size})),disc_entries:plan.disc_entries},system,copied);
    const hash=await mediaIdentity(media);checkAbort(signal);
    const game={schema_version:1,id:crypto.randomUUID(),title,system,content_hash:hash,byte_length:copied,region:null,nominal_refresh_hz:null,created_at_ms:Date.now(),media};
    const unique=new Map();for(const asset of prepared){requireValue(!unique.has(asset.content_hash)||unique.get(asset.content_hash).blob.size===asset.blob.size,'corrupt_data','One content identity has inconsistent byte lengths.');unique.set(asset.content_hash,asset);}
    // Admission and publication share one strict transaction. Every stored Blob
    // contains read/copied bytes rather than a reference to a mutable source File.
    checkAbort(signal);job.value.progress.phase='committing';job.committing=true;clearTimeout(job.timer);
    return this.#store.transaction(['config','games','assets'],'readwrite',async tx=>{
      const usage=validateUsage((await tx.get('config','usage'))?.value),existing=await tx.request(tx.store('games').index('content_hash').get(hash));
      if(existing)requireValue(existing.system===system&&existing.byte_length===copied,'conflict','This content is already registered with another system or length.');
      let added=0;for(const[contentHash,asset]of unique){const present=await tx.get('assets',contentHash);requireValue(!present||present.hash===contentHash&&present.bytes===asset.blob.size,'corrupt_data','Stored content accounting is inconsistent.');if(!present)added+=asset.blob.size;}
      requireValue(usage.game_count+(existing?0:1)<=50000&&usage.assets_bytes+added<=56*1024**3,'resource_limit','The browser library exceeds its configured storage bound.');
      for(const[contentHash,asset]of unique)await tx.put('assets',{hash:contentHash,bytes:asset.blob.size,blob:asset.blob});
      const stored=existing?{...existing,title,sort_title:normalized(title),media}: {...game,sort_title:normalized(title),last_played_at_ms:null,playtime_ms:0,favorite:0};await tx.put('games',stored);
      await tx.put('config',{key:'usage',value:{...usage,game_count:usage.game_count+(existing?0:1),assets_bytes:usage.assets_bytes+added}});return manifest(stored);
    },signal);
  }
  status(id){const job=this.#jobs.get(id);requireValue(job,'not_found','Import job is missing or has expired.');return structuredClone(job.value);}
  cancel(id){const job=this.#jobs.get(id);requireValue(job,'not_found','Import job is missing or has expired.');const accepted=job.value.state==='running'&&!job.committing;if(accepted)job.controller.abort();return{...structuredClone(job.value),cancel_accepted:accepted};}
  close(){for(const job of this.#jobs.values()){clearTimeout(job.timer);job.controller.abort();}this.#jobs.clear();}
  async asset(gameId,name=null){const game=await this.get(gameId),asset=game.media?.assets.find(item=>item.name===(name??game.media.entry_name));requireValue(asset,'not_found','The selected content asset is absent.');const stored=await this.#store.transaction(['assets'],'readonly',tx=>tx.get('assets',asset.content_hash));requireValue(stored?.blob instanceof Blob&&stored.bytes===asset.byte_length&&stored.blob.size===asset.byte_length,'corrupt_data','Stored game content is incomplete.');return{blob:stored.blob,hash:asset.content_hash,bytes:asset.byte_length};}
  async remove(id,{signal}={}){
    requireValue(UUID.test(id),'invalid_request','Game identity is invalid.');return this.#store.transaction(['config','games','saves','leases'],'readwrite',async tx=>{
      requireValue(await tx.get('games',id),'not_found','The game was not found.');const count=await tx.request(tx.store('saves').index('game').count(id));requireValue(count===0,'conflict','Retain or export saved progress before removing its library entry.');
      const leases=await tx.request(tx.store('leases').index('game').getAll(id));requireValue(!leases.some(item=>['ready','running','paused'].includes(item.info.state)&&item.expires_at_ms>Date.now()),'busy','Stop active game sessions before removing this library entry.');
      const optionKeys=await tx.request(tx.store('config').getAllKeys(IDBKeyRange.bound(`core_options.${id}.`,`core_options.${id}.\uffff`),33));requireValue(optionKeys.length<=32,'corrupt_data','Core option profile storage needs recovery.');for(const key of optionKeys)await tx.delete('config',key);
      const usage=validateUsage((await tx.get('config','usage'))?.value);requireValue(usage.game_count>0,'corrupt_data','Library accounting is inconsistent.');await tx.delete('games',id);await tx.put('config',{key:'usage',value:{...usage,game_count:usage.game_count-1}});return{removed:true};
    },signal);
  }
}
