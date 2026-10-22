import{WebPlatformError,UUID,HASH,requireValue,checkAbort,utf8}from'./errors.js';
import{SYNC_STORES,SYNC_VAULT_RECORD,clearSyncStorage}from'./sync-storage.js';
import{revokeSyncConsent}from'./sync-consent.js';

const RECORD=SYNC_VAULT_RECORD,KEY='sync.wrap-key.v1',CREDENTIAL_BYTES=84;
const exact=(value,fields)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===fields.length&&fields.every(field=>Object.hasOwn(value,field));
function publicAccount(value){
  requireValue(exact(value,['schema_version','account_id','device_id','root_public_key','audience','certificate','registered'])&&value.schema_version===1&&typeof value.account_id==='string'&&HASH.test(value.account_id)&&typeof value.device_id==='string'&&UUID.test(value.device_id)&&typeof value.root_public_key==='string'&&/^[A-Za-z0-9_-]{43}$/.test(value.root_public_key)&&typeof value.audience==='string'&&value.audience.length<=512&&typeof value.registered==='boolean'&&exact(value.certificate,['payload_base64','signature_base64'])&&typeof value.certificate.payload_base64==='string'&&value.certificate.payload_base64.length<=10932&&/^[A-Za-z0-9_-]+$/.test(value.certificate.payload_base64)&&typeof value.certificate.signature_base64==='string'&&/^[A-Za-z0-9_-]{86}$/.test(value.certificate.signature_base64),'corrupt_data','Browser synchronization identity is malformed.');
  let url;try{url=new URL(value.audience);}catch{throw new WebPlatformError('invalid_request','Use a canonical HTTPS synchronization service address.');}
  requireValue(url.protocol==='https:'&&!url.username&&!url.password&&!url.search&&!url.hash&&!value.audience.endsWith('/')&&url.origin+(url.pathname==='/'?'':url.pathname)===value.audience,'invalid_request','Use a canonical HTTPS synchronization service address.');return structuredClone(value);
}
function wrappingKey(value,profile){
  requireValue(exact(value,['schema_version','id','profile_id','key'])&&value.schema_version===1&&UUID.test(value.id)&&value.profile_id===profile&&value.key instanceof CryptoKey&&!value.key.extractable&&value.key.type==='secret'&&value.key.algorithm.name==='AES-GCM'&&value.key.algorithm.length===256&&value.key.usages.length===2&&value.key.usages.includes('encrypt')&&value.key.usages.includes('decrypt'),'corrupt_data','The origin-bound synchronization wrapping key is unavailable.');return value;
}
function record(value,profile){
  if(value===null)return null;
  requireValue(value&&value.schema_version===1&&value.profile_id===profile&&typeof value.generation==='string'&&UUID.test(value.generation)&&typeof value.connected==='boolean','corrupt_data','The browser synchronization vault needs recovery.');
  if(!value.connected){requireValue(exact(value,['schema_version','profile_id','generation','connected']),'corrupt_data','The disconnected synchronization vault is malformed.');return value;}
  requireValue(exact(value,['schema_version','profile_id','generation','connected','key_id','account','nonce','sealed'])&&UUID.test(value.key_id)&&value.nonce instanceof Uint8Array&&value.nonce.length===12&&value.sealed instanceof ArrayBuffer&&value.sealed.byteLength===CREDENTIAL_BYTES+16,'corrupt_data','The browser synchronization credential envelope is malformed.');publicAccount(value.account);return value;
}
function aad(profile,generation,keyId,account){return utf8(JSON.stringify({schema_version:1,format:'OPK1',profile_id:profile,generation,key_id:keyId,account_id:account.account_id,device_id:account.device_id,root_public_key:account.root_public_key,audience:account.audience}));}

// WebCrypto keys are origin-bound nonextractable IndexedDB CryptoKeys, not an OS
// vault or a defense against arbitrary trusted-origin script execution. Only the
// delegated device/data secrets are wrapped; recovery/root authority is absent.
export class WebSyncVault{
  #store;#profile;
  constructor(store,profile){requireValue(typeof profile==='string'&&UUID.test(profile),'invalid_request','Browser synchronization profile is invalid.');this.#store=store;this.#profile=profile;}
  async #snapshot(signal){return this.#store.transaction(['config'],'readonly',async tx=>({record:record((await tx.get('config',RECORD))?.value??null,this.#profile),key:(await tx.get('config',KEY))?.value??null}),signal);}
  async status({signal}={}){const value=(await this.#snapshot(signal)).record;return{generation:value?.generation??null,connected:value?.connected===true,account:value?.connected?publicAccount(value.account):null,credential_store:'origin_bound_webcrypto',root_key_retained:false};}
  async assertGeneration(generation,{signal}={}){const value=(await this.#snapshot(signal)).record;requireValue(value?.connected&&value.generation===generation,'permission_expired','Synchronization credentials changed. Unlock the current device again.');return publicAccount(value.account);}
  async store(account,credentials,expectedGeneration,{signal}={}){
    checkAbort(signal);requireValue(isSecureContext&&crypto.subtle&&credentials instanceof ArrayBuffer&&credentials.byteLength===CREDENTIAL_BYTES&&(expectedGeneration===null||typeof expectedGeneration==='string'&&UUID.test(expectedGeneration)),'invalid_request','The browser credential commit is invalid.');const bytes=new Uint8Array(credentials);
    try{
      requireValue(bytes[0]===79&&bytes[1]===80&&bytes[2]===75&&bytes[3]===49,'invalid_request','Browser credentials use an unsupported format.');const identity=publicAccount(account);requireValue(Array.from(bytes.subarray(4,20),value=>value.toString(16).padStart(2,'0')).join('')===identity.device_id.replaceAll('-',''),'invalid_request','Browser delegated credentials belong to another device.');const snapshot=await this.#snapshot(signal);requireValue((snapshot.record?.generation??null)===expectedGeneration,'conflict','Browser synchronization credentials changed before saving.');
      const priorKey=snapshot.key?wrappingKey(snapshot.key,this.#profile):null,key=priorKey??{schema_version:1,id:crypto.randomUUID(),profile_id:this.#profile,key:await crypto.subtle.generateKey({name:'AES-GCM',length:256},false,['encrypt','decrypt'])};
      const generation=crypto.randomUUID(),nonce=crypto.getRandomValues(new Uint8Array(12)),sealed=await crypto.subtle.encrypt({name:'AES-GCM',iv:nonce,additionalData:aad(this.#profile,generation,key.id,identity),tagLength:128},key.key,credentials);checkAbort(signal);
      const value={schema_version:1,profile_id:this.#profile,generation,connected:true,key_id:key.id,account:identity,nonce,sealed};
      await this.#store.transaction(['config'],'readwrite',async tx=>{
        const current=record((await tx.get('config',RECORD))?.value??null,this.#profile),currentKey=(await tx.get('config',KEY))?.value??null;
        requireValue((current?.generation??null)===expectedGeneration&&(currentKey?.id??null)===(priorKey?.id??null),'conflict','Browser synchronization credentials changed before commit.');if(currentKey)wrappingKey(currentKey,this.#profile);
        await tx.put('config',{key:KEY,value:key});await tx.put('config',{key:RECORD,value});
      },signal);return{generation,account:identity};
    }finally{bytes.fill(0);}
  }
  async unlock(expectedGeneration,{signal}={}){
    checkAbort(signal);const snapshot=await this.#snapshot(signal),value=snapshot.record;requireValue(value?.connected&&value.generation===expectedGeneration,'permission_expired','The connected browser synchronization device changed.');const key=wrappingKey(snapshot.key,this.#profile);requireValue(key.id===value.key_id,'corrupt_data','Browser credential wrapping identity changed.');let credentials;
    try{credentials=await crypto.subtle.decrypt({name:'AES-GCM',iv:value.nonce,additionalData:aad(this.#profile,value.generation,key.id,value.account),tagLength:128},key.key,value.sealed);checkAbort(signal);requireValue(credentials.byteLength===CREDENTIAL_BYTES,'corrupt_data','Browser delegated credentials have an invalid size.');await this.assertGeneration(value.generation,{signal});return{generation:value.generation,account:publicAccount(value.account),credentials};}
    catch(error){if(credentials)new Uint8Array(credentials).fill(0);if(error instanceof WebPlatformError)throw error;throw new WebPlatformError('permission_denied','Browser synchronization credentials could not be unwrapped. Local progress remains available.');}
  }
  async updateAccount(account,expectedGeneration,{signal}={}){
    // Certificate/registration updates also rewrap against a fresh generation.
    // A concurrent disconnect cannot be undone by a late account action.
    const unlocked=await this.unlock(expectedGeneration,{signal});try{const value=publicAccount(account);requireValue(['account_id','device_id','root_public_key','audience'].every(key=>value[key]===unlocked.account[key]),'conflict','A certificate update cannot replace its connected account.');return await this.store(value,unlocked.credentials,expectedGeneration,{signal});}finally{new Uint8Array(unlocked.credentials).fill(0);}
  }
  async disconnect(expectedGeneration,{signal}={}){
    requireValue(expectedGeneration===null||typeof expectedGeneration==='string'&&UUID.test(expectedGeneration),'invalid_request','The browser synchronization generation is invalid.');const generation=crypto.randomUUID();
    await this.#store.transaction(['config',...SYNC_STORES],'readwrite',async tx=>{const current=record((await tx.get('config',RECORD))?.value??null,this.#profile);requireValue((current?.generation??null)===expectedGeneration,'conflict','Browser synchronization credentials changed before disconnect.');await tx.put('config',{key:RECORD,value:{schema_version:1,profile_id:this.#profile,generation,connected:false}});await tx.delete('config',KEY);await clearSyncStorage(tx);await revokeSyncConsent(tx,this.#profile);},signal);return{generation,connected:false};
  }
}
