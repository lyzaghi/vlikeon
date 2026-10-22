import{UUID,HASH,integer,requireValue,utf8}from'./errors.js';
import{Sha256}from'./sha256.js';
import{originMetadata,checkedSaveOrigin,originMetadataHash}from'../save-origin.js';

export const SYNC_SAVE_STORES=Object.freeze(['saves','save_origins','save_payload_revisions','removed_saves']);
export async function syncRemoval(tx,id){
  requireValue(typeof id==='string'&&UUID.test(id),'invalid_request','The removed save identity is invalid.');const value=await tx.get('removed_saves',id);if(!value)return null;
  requireValue(value&&typeof value==='object'&&Object.keys(value).length===3&&value.id===id&&typeof value.game_content_hash==='string'&&HASH.test(value.game_content_hash)&&integer(value.removed_at_ms,0,Number.MAX_SAFE_INTEGER),'corrupt_data','The local save removal record needs recovery.');return value;
}
export function syncMetadataHash(input,canonical){
  const value=originMetadata(input);if(canonical)return originMetadataHash(value);
  const omitted=new Set(['game_id','conflict','generation','parent_id']),ordered=Object.fromEntries(Object.keys(value).filter(key=>!omitted.has(key)).sort().map(key=>[key,value[key]]));return new Sha256().update(utf8(JSON.stringify(ordered))).hex();
}
export async function syncSaveMetadata(tx,id){
  requireValue(typeof id==='string'&&UUID.test(id),'invalid_request','The synchronization save identity is invalid.');const saved=await tx.get('saves',id);requireValue(saved&&!await tx.get('removed_saves',id),'not_found','The local saved progress was removed before synchronization.');const{payload,...raw}=saved,projection=originMetadata(raw),revision=await tx.get('save_payload_revisions',id);
  requireValue(revision?.save_id===id&&typeof revision.revision==='string'&&UUID.test(revision.revision),'corrupt_data','Saved progress payload ownership needs recovery.');
  const origin=checkedSaveOrigin(await tx.get('save_origins',id),projection),canonical=origin.kind!=='legacy_projection',metadata=origin.canonical_metadata??projection,metadataHash=syncMetadataHash(metadata,canonical);requireValue(HASH.test(metadataHash),'corrupt_data','The synchronization save fingerprint is invalid.');return{projection,metadata,payload,canonical,metadata_hash:metadataHash,payload_revision:revision.revision};
}
export async function syncSaveIdentity(tx,id){const value=await syncSaveMetadata(tx,id);requireValue(value.payload instanceof Blob&&value.payload.size===value.projection.payload_bytes,'corrupt_data','Saved progress payload bytes need recovery.');return value;}
