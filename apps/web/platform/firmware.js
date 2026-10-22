import{HASH,UUID,integer,requireValue,checkAbort,fail}from'./errors.js';
import{hashInWorker}from'./hash-client.js';
import{firmwareRoleSizes}from'../runtime/firmware.js';

const MAX_BYTES=524288,MAX_OBJECTS=128,MAX_TOTAL=MAX_BYTES*MAX_OBJECTS,MAX_ROLES=8;
const STORES=['firmware_metadata','firmware_data','firmware_selections'];
function fields(value,keys){return value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));}
function hash(value){requireValue(typeof value==='string'&&value.length===64&&HASH.test(value),'invalid_request','Firmware identity is invalid.');return value;}
function role(system,value){try{return firmwareRoleSizes(system,value);}catch{fail('unsupported_capability','This firmware role is unsupported.');}}
function metadata(value){
  requireValue(fields(value,['content_hash','byte_length','created_at_ms','revision'])&&typeof value.content_hash==='string'&&value.content_hash.length===64&&HASH.test(value.content_hash)&&integer(value.byte_length,1,MAX_BYTES)&&integer(value.created_at_ms,0,Number.MAX_SAFE_INTEGER)&&typeof value.revision==='string'&&value.revision.length===36&&UUID.test(value.revision),'corrupt_data','Stored firmware metadata needs recovery.');return value;
}
function data(value,asset){requireValue(fields(value,['content_hash','revision','blob'])&&value.content_hash===asset.content_hash&&value.revision===asset.revision&&value.blob instanceof Blob&&value.blob.size===asset.byte_length,'corrupt_data','Stored firmware content is incomplete. Reimport its original file.');return value.blob;}
function selection(value){
  requireValue(fields(value,['key','content_hash'])&&typeof value.key==='string'&&value.key.length<=128&&typeof value.content_hash==='string'&&value.content_hash.length===64&&HASH.test(value.content_hash),'corrupt_data','Stored firmware selections need recovery.');const parts=value.key.split(':');
  requireValue(parts.length===2,'corrupt_data','Stored firmware role is invalid.');let sizes;try{sizes=firmwareRoleSizes(parts[0],parts[1]);}catch{fail('corrupt_data','Stored firmware role is unsupported.');}return{...value,sizes};
}
function publicAsset(asset,selected){return{content_hash:asset.content_hash,byte_length:asset.byte_length,created_at_ms:asset.created_at_ms,selected_roles:selected.filter(item=>item.content_hash===asset.content_hash).map(item=>item.key).sort()};}

export class WebFirmware{
  #store;#mutating=false;#reads=0;
  constructor(store){this.#store=store;}
  async #inventory(tx){
    const rows=await tx.request(tx.store('firmware_metadata').getAll(undefined,MAX_OBJECTS+1)),selected=await tx.request(tx.store('firmware_selections').getAll(undefined,MAX_ROLES+1));
    requireValue(rows.length<=MAX_OBJECTS&&selected.length<=MAX_ROLES,'corrupt_data','Firmware storage exceeds its inventory limit.');
    const assets=rows.map(metadata),selections=selected.map(selection),byHash=new Map(assets.map(item=>[item.content_hash,item]));
    requireValue(byHash.size===assets.length&&assets.reduce((total,item)=>total+item.byte_length,0)<=MAX_TOTAL,'corrupt_data','Firmware storage accounting is inconsistent.');
    for(const selected of selections){const asset=byHash.get(selected.content_hash);requireValue(asset&&selected.sizes.includes(asset.byte_length),'corrupt_data','A firmware selection references missing or incompatible content.');}
    return{assets,selections};
  }
  async list({signal}={}){
    return this.#store.transaction(['firmware_metadata','firmware_selections'],'readonly',async tx=>{
      const{assets,selections}=await this.#inventory(tx);return assets.sort((a,b)=>b.created_at_ms-a.created_at_ms||(a.content_hash<b.content_hash?-1:a.content_hash>b.content_hash?1:0)).map(asset=>publicAsset(asset,selections));
    },signal);
  }
  async usage({signal}={}){return this.#store.transaction(['firmware_metadata','firmware_selections'],'readonly',async tx=>{const{assets}=await this.#inventory(tx);return{count:assets.length,bytes:assets.reduce((total,item)=>total+item.byte_length,0)};},signal);}
  async #mutate(operation){requireValue(!this.#mutating,'busy','Wait for the current firmware operation to finish.');this.#mutating=true;try{return await operation();}finally{this.#mutating=false;}}
  async import(system,value,file,{signal}={}){
    const sizes=role(system,value);requireValue(file instanceof Blob&&sizes.includes(file.size),'content_mismatch','Selected firmware has the wrong size for this role.');
    return this.#mutate(async()=>{
      checkAbort(signal);const bytes=await file.arrayBuffer();checkAbort(signal);requireValue(bytes.byteLength===file.size&&bytes.byteLength<=MAX_BYTES,'corrupt_data','The selected firmware changed while being read.');
      // Hash the same immutable snapshot that will enter storage, never a second
      // read of a mutable provider File after its checksum was calculated.
      const blob=new Blob([bytes],{type:'application/octet-stream'}),contentHash=hash(await hashInWorker(blob,{signal}));checkAbort(signal);
      const revision=crypto.randomUUID(),now=Date.now(),key=`${system}:${value}`;
      return this.#store.transaction(STORES,'readwrite',async tx=>{
        const{assets,selections}=await this.#inventory(tx),previous=assets.find(item=>item.content_hash===contentHash);
        if(previous)requireValue(previous.byte_length===blob.size,'content_mismatch','Firmware length differs from an existing content identity.');
        else requireValue(assets.length<MAX_OBJECTS&&assets.reduce((total,item)=>total+item.byte_length,0)+blob.size<=MAX_TOTAL,'resource_limit','The local firmware vault is full. Delete unused versions before importing another.');
        const asset={content_hash:contentHash,byte_length:blob.size,created_at_ms:previous?.created_at_ms??now,revision};
        // Reimport can repair a missing/corrupt Blob with verified bytes of the
        // same hash. Rotate its revision so concurrent admissions must retry.
        await tx.put('firmware_data',{content_hash:contentHash,revision,blob});await tx.put('firmware_metadata',asset);await tx.put('firmware_selections',{key,content_hash:contentHash});
        return publicAsset(asset,[...selections.filter(item=>item.key!==key),{key,content_hash:contentHash}]);
      },signal);
    });
  }
  async #verified(contentHash,sizes,{signal}={}){
    const record=await this.#store.transaction(['firmware_metadata','firmware_data'],'readonly',async tx=>{
      const raw=await tx.get('firmware_metadata',contentHash);requireValue(raw,'not_found','The selected firmware is no longer stored.');const asset=metadata(raw);requireValue(asset.content_hash===contentHash,'corrupt_data','Stored firmware identity is inconsistent.');requireValue(sizes.includes(asset.byte_length),'content_mismatch','Firmware size does not match this role.');const blob=data(await tx.get('firmware_data',contentHash),asset);return{asset,blob};
    },signal);
    requireValue(await hashInWorker(record.blob,{signal})===contentHash,'corrupt_data','Stored firmware failed its checksum. Reimport the original file.');checkAbort(signal);return record;
  }
  async select(system,value,contentHash,{signal}={}){
    hash(contentHash);const sizes=role(system,value);return this.#mutate(async()=>{
      const{asset}=await this.#verified(contentHash,sizes,{signal});
      return this.#store.transaction(STORES,'readwrite',async tx=>{
        await this.#inventory(tx);const current=await tx.get('firmware_metadata',contentHash);requireValue(current&&metadata(current).revision===asset.revision,'conflict','Firmware changed before it could be selected. Retry the selection.');data(await tx.get('firmware_data',contentHash),asset);
        await tx.put('firmware_selections',{key:`${system}:${value}`,content_hash:contentHash});return{selected:true};
      },signal);
    });
  }
  async forget(contentHash,{signal}={}){
    hash(contentHash);return this.#mutate(()=>this.#store.transaction(STORES,'readwrite',async tx=>{
      const keys=await tx.request(tx.store('firmware_selections').index('hash').getAllKeys(IDBKeyRange.only(contentHash),MAX_ROLES+1));requireValue(keys.length<=MAX_ROLES,'corrupt_data','Firmware selection storage needs recovery.');for(const key of keys)await tx.delete('firmware_selections',key);
      await tx.delete('firmware_data',contentHash);await tx.delete('firmware_metadata',contentHash);return{forgotten:true};
    },signal));
  }
  async selected(gameId,value,expectedHash,{signal}={}){
    requireValue(typeof gameId==='string'&&gameId.length===36&&UUID.test(gameId),'invalid_request','Game identity is invalid.');hash(expectedHash);
    requireValue(this.#reads<4,'busy','Wait for a firmware read to finish.');this.#reads++;
    try{
      const initial=await this.#store.transaction(['games','firmware_selections'],'readonly',async tx=>{
        const game=await tx.get('games',gameId);requireValue(game,'not_found','The game is not in this library.');const sizes=role(game.system,value),key=`${game.system}:${value}`,selected=await tx.get('firmware_selections',key);
        requireValue(selected,'not_found','Import and select the required firmware in Settings.');requireValue(selection(selected).content_hash===expectedHash,'conflict','The selected firmware changed before opening. Retry the game.');return{game,key,sizes};
      },signal);
      const record=await this.#verified(expectedHash,initial.sizes,{signal});
      await this.#store.transaction(['games',...STORES],'readonly',async tx=>{
        const game=await tx.get('games',gameId),chosen=await tx.get('firmware_selections',initial.key),current=await tx.get('firmware_metadata',expectedHash);
        requireValue(game&&game.system===initial.game.system&&game.content_hash===initial.game.content_hash&&chosen&&selection(chosen).content_hash===expectedHash&&current&&metadata(current).revision===record.asset.revision,'conflict','The game or selected firmware changed during verification. Reopen the game.');data(await tx.get('firmware_data',expectedHash),record.asset);
      },signal);return{role:value,asset:record.asset,blob:record.blob};
    }finally{this.#reads--;}
  }
}
