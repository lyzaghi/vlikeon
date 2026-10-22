import{requireValue,utf8}from'./errors.js';
import{checkSyncLease}from'./sync-lease.js';
import{assertSyncContext,assertLocalExportContext}from'./sync-consent.js';
import{syncMetadataHash,syncSaveMetadata}from'./sync-save-identity.js';
import{sameHeader}from'./sync-network-policy.js';
import{validateSave}from'./saves.js';
import{originMetadataHash,sameSaveProjection}from'../save-origin.js';
import{hashInWorker}from'./hash-client.js';

const wipe=buffer=>{if(buffer instanceof ArrayBuffer&&buffer.byteLength)new Uint8Array(buffer).fill(0);};

// A locally sealed upload is a local backup, never authority to replace an
// origin ledger or recreate a missing save record. Its original bytes survive
// an ordinary retry; a different known canonical envelope needs retirement of
// its owned partial after the local source has been verified.
export async function recoverRetainedUpload({saves,journal,transport,credentials,lease,context}){
  const signal=checkSyncLease(lease,context.profile_id),slot=await journal.uploadSnapshot({requireSource:false});if(!slot)return{repaired:false,rebound:false,retired_for_replacement:false};let encrypted=null,opened=null;
  try{
    const guardTx=async tx=>{await assertSyncContext(tx,context);const current=await tx.get('sync_slots','upload');requireValue(current?.id===slot.id&&current.metadata_hash===slot.metadata_hash&&current.canonical===slot.canonical&&current.payload_revision===slot.payload_revision&&sameHeader(current.header,slot.header),'conflict','The retained encrypted upload owner changed during local recovery.');};
    const before=await saves.syncMetadata(slot.header.binding.object_id,{signal,guardTx});encrypted=await slot.payload.arrayBuffer();checkSyncLease(lease,context.profile_id);opened=await credentials.open(slot.header,encrypted,{signal});checkSyncLease(lease,context.profile_id);const metadata=validateSave(opened.value);
    requireValue(metadata.id===slot.header.binding.object_id&&opened.binary instanceof ArrayBuffer&&opened.binary.byteLength===metadata.payload_bytes&&slot.metadata_hash===syncMetadataHash(metadata,slot.canonical),'corrupt_data','The retained encrypted upload does not match its original save fingerprint.');requireValue(sameSaveProjection(before.projection,metadata),'conflict','The retained encrypted upload belongs to different local saved progress.');
    if(slot.canonical)requireValue(before.canonical&&before.metadata_hash===originMetadataHash(metadata),'conflict','The retained upload canonical origin is missing or changed.');else requireValue(syncMetadataHash(before.metadata,false)===slot.metadata_hash,'conflict','The retained legacy upload projection changed.');
    const payload=new Blob([opened.binary],{type:'application/octet-stream'});wipe(opened.binary);const guarded=async tx=>{await guardTx(tx);const current=await syncSaveMetadata(tx,metadata.id);requireValue(originMetadataHash(current.projection)===originMetadataHash(before.projection)&&current.canonical===before.canonical&&current.metadata_hash===before.metadata_hash,'conflict','Saved progress metadata or origin changed during retained upload recovery.');};
    const result=await saves.repairStaged(metadata,payload,{signal,guardTx:guarded});requireValue(result,'not_found','The staged generation was removed locally.');const source=await saves.forSync(metadata.id,{signal});checkSyncLease(lease,context.profile_id);
    if(source.canonical&&originMetadataHash(metadata)!==source.metadata_hash){
      requireValue(!slot.canonical&&sameSaveProjection(source.projection,metadata)&&syncMetadataHash(source.metadata,false)===slot.metadata_hash,'conflict','The retained upload cannot be replaced by this canonical generation.');await transport.retireUpload(slot.id,{source});return{repaired:result.repaired,rebound:false,retired_for_replacement:true};
    }
    const rebound=source.canonical!==slot.canonical||source.metadata_hash!==slot.metadata_hash||source.payload_revision!==slot.payload_revision;if(rebound)await journal.bindUpload(slot.id,metadata,source);return{repaired:result.repaired,rebound,retired_for_replacement:false};
  }finally{wipe(encrypted);wipe(opened?.binary);}
}

// Explicit local export only. A missing local generation/origin is not restored
// or promoted: the authenticated backup is handed to an ordinary OPSV import.
export async function exportRetainedUpload({store,snapshot,credentials,lease,context,deliver}){
  const signal=checkSyncLease(lease,context.profile_id);requireValue(typeof deliver==='function'&&typeof snapshot==='function','invalid_request','Choose a destination for the pending backup.');const slot=await snapshot();requireValue(slot,'not_found','There is no pending encrypted backup to export.');let encrypted=null,opened=null;
  const guard=async()=>{checkSyncLease(lease,context.profile_id);await store.transaction(['config','sync_slots','removed_saves'],'readonly',async tx=>{await assertLocalExportContext(tx,context);const current=await tx.get('sync_slots','upload');requireValue(current?.id===slot.id&&current.metadata_hash===slot.metadata_hash&&current.canonical===slot.canonical&&current.payload_revision===slot.payload_revision&&sameHeader(current.header,slot.header),'conflict','The pending backup changed before export completed.');requireValue(!await tx.get('removed_saves',slot.header.binding.object_id),'permission_denied','This saved generation was explicitly removed. Its pending copy cannot be exported through synchronization.');},signal);checkSyncLease(lease,context.profile_id);};
  try{
    await guard();encrypted=await slot.payload.arrayBuffer();checkSyncLease(lease,context.profile_id);opened=await credentials.open(slot.header,encrypted,{signal});await guard();const metadata=validateSave(opened.value);
    requireValue(metadata.id===slot.header.binding.object_id&&opened.binary instanceof ArrayBuffer&&opened.binary.byteLength===metadata.payload_bytes&&syncMetadataHash(metadata,slot.canonical)===slot.metadata_hash,'corrupt_data','The pending encrypted backup does not match its saved generation.');const payload=new Blob([opened.binary],{type:'application/octet-stream'});wipe(opened.binary);requireValue(await hashInWorker(payload,{signal})===metadata.payload_hash,'corrupt_data','The pending saved progress failed its actual-byte checksum.');await guard();const json=utf8(JSON.stringify(metadata)),header=new Uint8Array(12);header.set(utf8('OPSV0001'));new DataView(header.buffer).setUint32(8,json.length,true);return await deliver(new Blob([header,json,payload],{type:'application/octet-stream'}),signal,guard);
  }finally{wipe(encrypted);wipe(opened?.binary);}
}
