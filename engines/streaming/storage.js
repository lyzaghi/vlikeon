import{StreamError,requireValue,newIdentity,validateIdentity,UUID,verify,DOMAINS,validGrant}from'./crypto.js';
import{defaultRemoteControls,validateRemoteControls}from'./control-profile.js';
const request=input=>new Promise((resolve,reject)=>{input.onsuccess=()=>resolve(input.result);input.onerror=()=>reject(input.error);});
export async function openStreamingStore(onClosed=()=>{}){
  const opening=indexedDB.open('omniplay-streaming-v1',1);let abandoned=false;
  opening.onupgradeneeded=()=>{if(abandoned){opening.transaction.abort();return;}const db=opening.result;db.createObjectStore('identity');db.createObjectStore('pairs',{keyPath:'id'});};
  const db=await new Promise((resolve,reject)=>{opening.onerror=()=>reject(new StreamError('storage_failure','Remote-play storage could not open.'));opening.onblocked=()=>{abandoned=true;reject(new StreamError('busy','Close other OmniPlay windows before updating remote-play storage.'));};opening.onsuccess=()=>{if(abandoned)opening.result.close();else resolve(opening.result);};});
  db.onversionchange=()=>{db.close();onClosed();};const store=new StreamingStore(db);
  try{const existing=await store.identity();if(existing)await validateIdentity(existing);else{const identity=await newIdentity();await store.initializeIdentity(identity);}return store;}catch(error){db.close();throw error;}
}
class StreamingStore{
  #db;
  constructor(db){this.#db=db;}
  async transaction(names,mode,body){
    let tx;try{tx=this.#db.transaction(names,mode,mode==='readwrite'?{durability:'strict'}:undefined);}catch{throw new StreamError('storage_failure','Remote-play storage is unavailable.');}
    if(mode==='readwrite'&&tx.durability!=='strict'){tx.abort();throw new StreamError('unsupported_capability','This browser lacks the required strict storage transaction policy.');}
    const complete=new Promise((resolve,reject)=>{tx.oncomplete=resolve;tx.onabort=()=>reject(new StreamError('storage_failure','Remote-play storage did not commit.'));tx.onerror=()=>{};});void complete.catch(()=>{});
    const context={get:(name,key)=>request(tx.objectStore(name).get(key)),put:(name,value,key)=>request(tx.objectStore(name).put(value,key)),all:name=>request(tx.objectStore(name).getAll()),delete:(name,key)=>request(tx.objectStore(name).delete(key))};
    try{const value=await body(context);await complete;return value;}catch(error){try{tx.abort();}catch{}await complete.catch(()=>{});throw error;}
  }
  identity(){return this.transaction(['identity'],'readonly',tx=>tx.get('identity','device'));}
  initializeIdentity(identity){return this.transaction(['identity'],'readwrite',async tx=>{if(!await tx.get('identity','device'))await tx.put('identity',identity,'device');});}
  async savePair(record){
    requireValue(UUID.test(record.id)&&record.key instanceof CryptoKey&&!record.key.extractable&&record.key.algorithm.name==='AES-GCM'&&record.key.algorithm.length===256,'corrupt_data','Pairing encryption key is invalid.');
    const grant=await verify(record.host_public_key,DOMAINS.grant,record.grant,8192);validGrant(grant,record.audience);
    requireValue(record.id===grant.id&&record.host_public_key===grant.host_public_key&&record.client_public_key===grant.client_public_key&&['host','client'].includes(record.role),'corrupt_data','Pairing record is invalid.');
    return this.transaction(['pairs'],'readwrite',async tx=>{
      const previous=await tx.get('pairs',record.id);if(previous){requireValue(!previous.revoked&&previous.grant.payload===record.grant.payload&&previous.grant.signature===record.grant.signature&&previous.role===record.role,'conflict','Pairing identity already exists with different authorization.');return;}
      const all=await tx.all('pairs');requireValue(all.filter(item=>!item.revoked||!item.relay_revoked).length<64,'resource_limit','Pairing storage is full.');for(const old of all)if(old.revoked&&old.relay_revoked)await tx.delete('pairs',old.id);
      await tx.put('pairs',{...record,send_sequence:0,receive_sequence:0,cursor:0,revoked:false,relay_confirmed:false,lease:null});
    });
  }
  async pair(id){const record=await this.transaction(['pairs'],'readonly',tx=>tx.get('pairs',id));requireValue(record&&!record.revoked&&record.key instanceof CryptoKey&&!record.key.extractable&&record.key.algorithm.name==='AES-GCM','not_found','Remote-play pairing is unavailable.');const grant=await verify(record.host_public_key,DOMAINS.grant,record.grant,8192);validGrant(grant,record.audience);const identity=await this.identity();requireValue(record.id===grant.id&&record.host_public_key===grant.host_public_key&&record.client_public_key===grant.client_public_key&&(record.role==='host'?grant.host_public_key:record.role==='client'?grant.client_public_key:null)===identity.public_key,'corrupt_data','Pairing identity does not match this device.');return record;}
  async list(){const all=await this.transaction(['pairs'],'readonly',tx=>tx.all('pairs'));return all.filter(value=>!value.revoked||!value.relay_revoked).map(({key,...record})=>record);}
  confirm(id){return this.transaction(['pairs'],'readwrite',async tx=>{const record=await tx.get('pairs',id);requireValue(record&&!record.revoked,'not_found','Pairing is unavailable.');await tx.put('pairs',{...record,relay_confirmed:true});});}
  revocation(id){return this.transaction(['pairs'],'readonly',tx=>tx.get('pairs',id));}
  revoke(id){return this.transaction(['pairs'],'readwrite',async tx=>{const record=await tx.get('pairs',id);if(record){const{key,control_profile,...rest}=record;await tx.put('pairs',{...rest,revoked:true,relay_revoked:record.relay_revoked===true,lease:null});}});}
  async controlProfile(id){const record=await this.pair(id);requireValue(record.role==='client','permission_denied','Control preferences belong to the receiving device.');return record.control_profile===undefined?defaultRemoteControls():validateRemoteControls(record.control_profile);}
  async setControlProfile(id,profile){
    const value=validateRemoteControls(profile),verified=await this.pair(id);
    requireValue(verified.role==='client','permission_denied','Control preferences belong to the receiving device.');
    return this.transaction(['pairs'],'readwrite',async tx=>{
      const record=await tx.get('pairs',id);
      requireValue(record&&!record.revoked&&record.role==='client'&&record.grant.payload===verified.grant.payload&&record.grant.signature===verified.grant.signature,
        'permission_expired','Pairing changed before the control preferences could be saved.');
      // Read/merge in the same transaction. Preference saves cannot restore an
      // older signaling sequence, receiver lease, key or revocation state.
      await tx.put('pairs',{...record,control_profile:value});return value;
    });
  }
  confirmRevocation(id){return this.transaction(['pairs'],'readwrite',async tx=>{const record=await tx.get('pairs',id);if(record?.revoked)await tx.put('pairs',{...record,relay_revoked:true});});}
  reserve(id){return this.transaction(['pairs'],'readwrite',async tx=>{const record=await tx.get('pairs',id);requireValue(record&&!record.revoked&&Number.isSafeInteger(record.send_sequence)&&record.send_sequence<1000000,'permission_expired','Pairing must be renewed before sending more signals.');const sequence=record.send_sequence+1;await tx.put('pairs',{...record,send_sequence:sequence});return sequence;});}
  lease(id,owner){return this.transaction(['pairs'],'readwrite',async tx=>{const record=await tx.get('pairs',id);requireValue(record&&!record.revoked,'not_found','Pairing is unavailable.');requireValue(!record.lease||record.lease.owner===owner||record.lease.expires_at_ms<=Date.now(),'busy','This pairing is already active in another OmniPlay window.');await tx.put('pairs',{...record,lease:{owner,expires_at_ms:Date.now()+30000}});});}
  release(id,owner){return this.transaction(['pairs'],'readwrite',async tx=>{const record=await tx.get('pairs',id);if(record?.lease?.owner===owner)await tx.put('pairs',{...record,lease:null});});}
  accept(id,owner,packet){return this.transaction(['pairs'],'readwrite',async tx=>{const record=await tx.get('pairs',id);requireValue(record&&!record.revoked&&record.lease?.owner===owner&&record.lease.expires_at_ms>Date.now(),'permission_expired','Remote-play receiver lease expired.');requireValue(packet.cursor>record.cursor&&packet.sequence>record.receive_sequence,'conflict','Signaling replay or sequence regression was rejected.');await tx.put('pairs',{...record,cursor:packet.cursor,receive_sequence:packet.sequence});});}
  close(){this.#db.close();}
}
