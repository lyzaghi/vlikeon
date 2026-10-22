import{requireValue,integer,UUID,HASH,utf8}from'./errors.js';
import{exact}from'./sync-network-policy.js';
import{syncSaveMetadata}from'./sync-save-identity.js';
import{originMetadataHash}from'../save-origin.js';

export const RECOVERY_LIMIT=64;
const states=['queued','downloading','waiting_for_game_close','recovered','already_intact','failed','cancelled'];
const uuid=value=>typeof value==='string'&&UUID.test(value),hash=value=>typeof value==='string'&&HASH.test(value);
export const recoveryTerminal=value=>['recovered','already_intact','failed','cancelled'].includes(value.status.state);
export function recoveryRecord(value,save=null){
  requireValue(exact(value,['schema_version','save_id','request_id','account_id','device_id','game_id','local_metadata_hash','canonical_hash','status','completion'])&&value.schema_version===1&&uuid(value.save_id)&&(save===null||value.save_id===save)&&uuid(value.request_id)&&value.request_id!=='00000000-0000-0000-0000-000000000000'&&hash(value.account_id)&&uuid(value.device_id)&&uuid(value.game_id)&&hash(value.local_metadata_hash)&&(value.canonical_hash===null||hash(value.canonical_hash))&&exact(value.status,['save_id','state','requested_at_ms','updated_at_ms','error'])&&value.status.save_id===value.save_id&&states.includes(value.status.state)&&integer(value.status.requested_at_ms,0,Number.MAX_SAFE_INTEGER)&&integer(value.status.updated_at_ms,value.status.requested_at_ms,Number.MAX_SAFE_INTEGER),'corrupt_data','The local cloud recovery request needs recovery.');
  requireValue(value.completion===null||['recovered','already_intact'].includes(value.status.state)&&exact(value.completion,['canonical_metadata_hash','ciphertext_sha256'])&&hash(value.completion.canonical_metadata_hash)&&hash(value.completion.ciphertext_sha256),'corrupt_data','The cloud recovery completion proof is malformed.');requireValue(value.status.state!=='recovered'||value.completion!==null,'corrupt_data','Authenticated cloud recovery completion evidence is missing.');
  const error=value.status.error;requireValue(error===null||exact(error,['code','message','recoverable'])&&typeof error.code==='string'&&/^[a-z_]{1,64}$/.test(error.code)&&typeof error.message==='string'&&utf8(error.message).length<=512&&error.recoverable===true,'corrupt_data','The cloud recovery error record is malformed.');requireValue(utf8(JSON.stringify(value)).length<=4096,'corrupt_data','The cloud recovery request exceeds its record bound.');return value;
}
export async function recoveryEntries(tx,account){
  const values=await tx.request(tx.store('sync_recovery').getAll(undefined,RECOVERY_LIMIT+1));requireValue(values.length<=RECOVERY_LIMIT,'corrupt_data','Cloud recovery request storage exceeds its bound.');return values.map(value=>recoveryRecord(value)).filter(value=>value.account_id===account).sort((a,b)=>a.status.requested_at_ms-b.status.requested_at_ms||a.save_id.localeCompare(b.save_id));
}
export async function assertRecoveryRequest(tx,request,save,context){
  const raw=await tx.get('sync_recovery',save);requireValue(raw,'cancelled','This cloud recovery request was cancelled or removed.');const current=recoveryRecord(raw,save);requireValue(current.request_id===request&&current.account_id===context.account_id&&current.device_id===context.device_id&&!recoveryTerminal(current),'cancelled','This cloud recovery request was cancelled or replaced.');return current;
}
export async function assertRecoveryLocal(tx,entry){
  const source=await syncSaveMetadata(tx,entry.save_id);requireValue(source.projection.game_id===entry.game_id&&originMetadataHash(source.projection)===entry.local_metadata_hash,'conflict','Saved progress metadata changed after recovery was requested.');if(entry.canonical_hash!==null)requireValue(source.canonical&&source.metadata_hash===entry.canonical_hash,'conflict','The canonical saved generation changed after recovery was requested.');return source;
}
export async function setRecoveryState(tx,expected,state,error=null,completion=null){
  requireValue(states.includes(state),'invalid_request','The cloud recovery state is invalid.');const current=await assertRecoveryRequest(tx,expected.request_id,expected.save_id,expected),next={...current,completion,status:{...current.status,state,updated_at_ms:Math.max(Date.now(),current.status.updated_at_ms),error}};recoveryRecord(next);await tx.put('sync_recovery',next);return next;
}
