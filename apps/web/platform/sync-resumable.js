import{checkSyncLease}from'./sync-lease.js';
import{requireValue,integer,UUID}from'./errors.js';
import{blobHeader,sameHeader}from'./sync-network-policy.js';
import{SYNC_CHUNK}from'./sync-journal.js';
import{SYNC_CRYPTO_BINARY}from'./sync-wasm-policy.js';

const wipe=buffer=>{if(buffer instanceof ArrayBuffer&&buffer.byteLength)new Uint8Array(buffer).fill(0);};
const downloadIdentity=(object,sequence,bytes)=>requireValue(typeof object==='string'&&UUID.test(object)&&(sequence===null||integer(sequence,1,Number.MAX_SAFE_INTEGER))&&(bytes===null||integer(bytes,16,SYNC_CRYPTO_BINARY)),'invalid_request','The encrypted download identity is invalid.');

// This layer moves only ciphertext. Decryption, canonical game/save mapping,
// conflict publication and inbox consumption belong to the save reconciler.
export class WebSyncResumable{
  #credentials;#http;#journal;#saves;#lease;#guard;#progress;#capabilities=false;#busy=false;
  constructor({credentials,http,journal,saves,lease,guard,onProgress=()=>{}}){requireValue(typeof guard==='function'&&typeof onProgress==='function','invalid_request','The synchronization opportunity guard is required.');checkSyncLease(lease,lease?.profile);this.#credentials=credentials;this.#http=http;this.#journal=journal;this.#saves=saves;this.#lease=lease;this.#guard=guard;this.#progress=onProgress;}
  async #check(){checkSyncLease(this.#lease,this.#lease.profile);await this.#guard();checkSyncLease(this.#lease,this.#lease.profile);}
  async #ready(){await this.#check();if(!this.#capabilities){await this.#http.capabilities({signal:this.#lease.signal});await this.#check();this.#capabilities=true;}}
  #report(direction,completed,total){try{this.#progress({direction,completed_bytes:completed,total_bytes:total});}catch{}}
  async #run(operation){requireValue(!this.#busy,'busy','Wait for the current encrypted transfer.');this.#busy=true;try{await this.#ready();return await operation();}finally{this.#busy=false;}}
  prepare(){return this.#run(async()=>({protocol:1,ready:true}));}
  async upload(id){requireValue(typeof id==='string'&&UUID.test(id),'invalid_request','The upload save identity is invalid.');return this.#run(async()=>{
    const signal=this.#lease.signal,source=await this.#saves.forSync(id,{signal});await this.#check();let slot=await this.#journal.uploadSnapshot();
    if(slot)requireValue(slot.header.binding.object_id===id&&slot.metadata_hash===source.metadata_hash&&slot.canonical===source.canonical,'busy','Resume the retained upload before another save.');
    else{
      let plaintext=null,encrypted=null;
      try{plaintext=await source.payload.arrayBuffer();await this.#check();encrypted=await this.#credentials.seal(source.metadata,plaintext,{signal});await this.#check();blobHeader(encrypted.value,null,id);slot=await this.#journal.stageUpload(encrypted.value,new Blob([encrypted.binary],{type:'application/octet-stream'}),source);}
      finally{wipe(plaintext);wipe(encrypted?.binary);}
    }
    await this.#check();await this.#journal.checkUpload(slot.id);let acknowledgement;
    try{acknowledgement=await this.#http.begin(slot.id,slot.header,{signal});}
    catch(error){if(error.code!=='not_found')throw error;await this.#check();slot=await this.#journal.renewUploadId(slot.id);acknowledgement=await this.#http.begin(slot.id,slot.header,{signal});}
    this.#report('upload',acknowledgement.received_bytes,slot.header.ciphertext_bytes);
    while(!acknowledgement.stored&&acknowledgement.received_bytes<slot.header.ciphertext_bytes){
      await this.#check();await this.#journal.checkUpload(slot.id);const offset=acknowledgement.received_bytes,chunk=slot.payload.slice(offset,Math.min(slot.header.ciphertext_bytes,offset+SYNC_CHUNK));acknowledgement=await this.#http.putChunk(slot.id,slot.header,offset,chunk,{signal});await this.#check();this.#report('upload',acknowledgement.received_bytes,slot.header.ciphertext_bytes);
    }
    if(!acknowledgement.stored){await this.#check();await this.#journal.checkUpload(slot.id);acknowledgement=await this.#http.commit(slot.id,slot.header,{signal});}
    await this.#check();return this.#journal.finishUpload(slot.id,acknowledgement);
  });}
  async download(object,sequence=null,expectedBytes=null,{recoveryRequest=null,guard=null}={}){downloadIdentity(object,sequence,expectedBytes);requireValue((recoveryRequest===null||typeof recoveryRequest==='string'&&UUID.test(recoveryRequest)&&sequence===null)&&(guard===null||typeof guard==='function'),'invalid_request','The encrypted download request guard is invalid.');return this.#run(()=>this.#download(object,sequence,expectedBytes,recoveryRequest,guard));}
  async #download(object,sequence,expectedBytes,recoveryRequest=null,guard=null){
    const check=async()=>{await this.#check();if(guard)await guard();await this.#check();};await check();downloadIdentity(object,sequence,expectedBytes);const signal=this.#lease.signal;let slot=await this.#journal.downloadSnapshot();if(slot)requireValue(slot.header.binding.object_id===object&&slot.sequence===sequence&&slot.recovery_request_id===recoveryRequest,'busy','Finish the retained download before another remote object.');
    const header=await this.#http.header(object,{signal});await check();requireValue(expectedBytes===null||expectedBytes===header.ciphertext_bytes,'corrupt_data','The remote listing and object header disagree on encrypted size.');if(slot)requireValue(sameHeader(slot.header,header),'conflict','The immutable remote object changed while a partial copy was retained.');else slot=await this.#journal.startDownload(header,sequence,{recoveryRequest});
    this.#report('download',slot.offset,header.ciphertext_bytes);
    while(slot.offset<header.ciphertext_bytes){await check();const offset=slot.offset,bytes=await this.#http.chunk(header,offset,{signal});await check();slot=await this.#journal.appendDownload(slot.id,offset,bytes);await check();this.#report('download',slot.offset,header.ciphertext_bytes);}
    await check();return this.#journal.publishDownload(slot.id);
  }
  async resumeDownload(){return this.#run(async()=>{const slot=await this.#journal.downloadSnapshot();requireValue(!slot||slot.recovery_request_id===null,'busy','Resume the retained download through its explicit cloud recovery request.');return slot?this.#download(slot.header.binding.object_id,slot.sequence,slot.header.ciphertext_bytes):null;});}
  async retireUpload(id,{source=null}={}){requireValue(typeof id==='string'&&UUID.test(id),'invalid_request','The upload transfer identity is invalid.');return this.#run(async()=>{const owner=await this.#journal.uploadOwner();requireValue(owner?.id===id,'conflict','The retained upload owner changed.');try{await this.#http.abort(id,{signal:this.#lease.signal});}catch(error){if(error.code!=='not_found')throw error;}await this.#check();return this.#journal.discardUpload(id,{source});});}
}
