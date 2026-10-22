import{checkAbort,requireValue,UUID,HASH}from'./errors.js';
import{SYNC_VAULT_RECORD}from'./sync-storage.js';

export const SYNC_CONSENT='sync.consent.v1';
const uuid=value=>typeof value==='string'&&UUID.test(value),hash=value=>typeof value==='string'&&HASH.test(value);
function consent(value,profile){
  if(value===null)return null;const fields=['schema_version','profile_id','epoch','enabled','account_id','device_id'];requireValue(value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===fields.length&&fields.every(key=>Object.hasOwn(value,key))&&value.schema_version===1&&value.profile_id===profile&&uuid(value.epoch)&&typeof value.enabled==='boolean'&&(value.enabled?hash(value.account_id)&&uuid(value.device_id):value.account_id===null&&value.device_id===null),'corrupt_data','Browser synchronization consent needs recovery.');return value;
}
function credential(value,profile){requireValue(value?.schema_version===1&&value.connected===true&&value.profile_id===profile&&uuid(value.generation)&&value.account?.registered===true&&hash(value.account.account_id)&&uuid(value.account.device_id),'permission_denied','Connect and register a synchronization account first.');return value;}
export async function revokeSyncConsent(tx,profile){
  const value={schema_version:1,profile_id:profile,epoch:crypto.randomUUID(),enabled:false,account_id:null,device_id:null};await tx.put('config',{key:SYNC_CONSENT,value});const row=await tx.get('config','settings');if(row?.value&&typeof row.value==='object'&&!Array.isArray(row.value))await tx.put('config',{key:'settings',value:{...row.value,cloud_sync_opt_in:false}});return value;
}
export async function readSyncConsent(tx,profile){
  requireValue(uuid(profile),'invalid_request','The synchronization consent profile is invalid.');
    const value=consent((await tx.get('config',SYNC_CONSENT))?.value??null,profile),vault=(await tx.get('config',SYNC_VAULT_RECORD))?.value,settings=(await tx.get('config','settings'))?.value;
    return{consent_protocol:1,consent_epoch:value?.epoch??null,credential_generation:vault?.generation??null,enabled:Boolean(value?.enabled&&vault?.connected===true&&vault.account?.registered===true&&vault.profile_id===profile&&value.account_id===vault.account.account_id&&value.device_id===vault.account.device_id&&settings?.schema_version===1&&settings.cloud_sync_opt_in===true)};
}
export async function syncConsentStatus(store,profile,{signal}={}){return store.transaction(['config'],'readonly',tx=>readSyncConsent(tx,profile),signal);}
export async function configureSyncConsent(store,profile,{enabled,generation=null,expected_epoch=null},normalize,{signal,available=false}={}){
  checkAbort(signal);requireValue(uuid(profile)&&typeof enabled==='boolean'&&(generation===null||uuid(generation))&&(expected_epoch===null||uuid(expected_epoch))&&typeof normalize==='function','invalid_request','The synchronization consent request is invalid.');if(enabled)requireValue(available,'unsupported_capability','Encrypted synchronization is not available under the current product policy.');
  return store.transaction(['config'],'readwrite',async tx=>{
    requireValue((await tx.get('config','profile_id'))?.value===profile,'permission_expired','The local browser profile changed.');if(!enabled){const value=await revokeSyncConsent(tx,profile);return{enabled:false,consent_epoch:value.epoch};}
    const vault=credential((await tx.get('config',SYNC_VAULT_RECORD))?.value,profile),previous=consent((await tx.get('config',SYNC_CONSENT))?.value??null,profile);requireValue(vault.generation===generation&&(previous?.epoch??null)===expected_epoch,'conflict','The account or synchronization consent changed. Refresh before enabling it.');const raw=(await tx.get('config','settings'))?.value,settings=normalize(raw);
    const value=previous?.enabled&&previous.account_id===vault.account.account_id&&previous.device_id===vault.account.device_id&&raw?.cloud_sync_opt_in===true?previous:{schema_version:1,profile_id:profile,epoch:crypto.randomUUID(),enabled:true,account_id:vault.account.account_id,device_id:vault.account.device_id};
    await tx.put('config',{key:'settings',value:{...settings,cloud_sync_opt_in:true}});await tx.put('config',{key:SYNC_CONSENT,value});return{enabled:true,consent_epoch:value.epoch};
  },signal);
}
export async function assertSyncContext(tx,context){
  requireValue(context&&uuid(context.profile_id)&&hash(context.account_id)&&uuid(context.device_id)&&uuid(context.generation)&&uuid(context.consent_epoch),'invalid_request','The synchronization opportunity identity is invalid.');const profile=(await tx.get('config','profile_id'))?.value,grant=consent((await tx.get('config',SYNC_CONSENT))?.value??null,context.profile_id),vault=credential((await tx.get('config',SYNC_VAULT_RECORD))?.value,context.profile_id),settings=(await tx.get('config','settings'))?.value;
  requireValue(profile===context.profile_id&&vault.generation===context.generation&&vault.account.account_id===context.account_id&&vault.account.device_id===context.device_id&&grant?.enabled===true&&grant.epoch===context.consent_epoch&&grant.account_id===context.account_id&&grant.device_id===context.device_id&&settings?.schema_version===1&&settings.cloud_sync_opt_in===true,'cancelled','Synchronization consent or account identity changed. Local saved progress remains available.');return vault.account;
}
export async function loadSyncContext(store,profile,{signal}={}){
  return store.transaction(['config'],'readonly',async tx=>{const vault=credential((await tx.get('config',SYNC_VAULT_RECORD))?.value,profile),value=consent((await tx.get('config',SYNC_CONSENT))?.value??null,profile);requireValue(value?.enabled,'cancelled','Synchronization consent was withdrawn before this opportunity acquired ownership.');const context={profile_id:profile,account_id:vault.account.account_id,device_id:vault.account.device_id,generation:vault.generation,consent_epoch:value.epoch};await assertSyncContext(tx,context);return context;},signal);
}
// A user-selected local export grants no network/upload/reconciliation consent.
// Keep a distinct purpose so it cannot be passed to normal sync admission.
export async function assertLocalExportContext(tx,context){
  const fields=['purpose','profile_id','account_id','device_id','generation'];requireValue(context&&typeof context==='object'&&!Array.isArray(context)&&Object.keys(context).length===fields.length&&fields.every(key=>Object.hasOwn(context,key))&&context.purpose==='local_pending_export_v1'&&uuid(context.profile_id)&&hash(context.account_id)&&uuid(context.device_id)&&uuid(context.generation),'invalid_request','The local backup export identity is invalid.');
  const profile=(await tx.get('config','profile_id'))?.value,vault=credential((await tx.get('config',SYNC_VAULT_RECORD))?.value,context.profile_id);requireValue(profile===context.profile_id&&vault.generation===context.generation&&vault.account.account_id===context.account_id&&vault.account.device_id===context.device_id,'cancelled','The local backup export account or profile changed.');return vault.account;
}
export async function loadLocalExportContext(store,profile,{signal}={}){requireValue(uuid(profile),'invalid_request','The local backup profile is invalid.');return store.transaction(['config'],'readonly',async tx=>{const vault=credential((await tx.get('config',SYNC_VAULT_RECORD))?.value,profile),context={purpose:'local_pending_export_v1',profile_id:profile,account_id:vault.account.account_id,device_id:vault.account.device_id,generation:vault.generation};await assertLocalExportContext(tx,context);return context;},signal);}
