import{Sha256}from'./runtime/sha256.js';
import{validCoreVersion}from'./runtime/core-resources.js';

const fields=['schema_version','id','game_id','game_content_hash','kind','slot','generation','parent_id','core_id','core_version','state_format_version','platform','core_config_hash','payload_hash','payload_bytes','created_at_ms','device_id','label','conflict'];
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const HASH=/^[0-9a-f]{64}$/;
const encode=value=>new TextEncoder().encode(value);
function check(value){if(!value)throw Object.assign(new Error('Saved progress origin metadata is inconsistent.'),{name:'SaveOriginError',code:'corrupt_data'});}
function text(value,max,empty=false){return typeof value==='string'&&(empty||value.length>0)&&encode(value).length<=max&&Array.from(value).every(c=>c.codePointAt(0)<0xd800||c.codePointAt(0)>0xdfff);}
const integer=(value,min,max=Number.MAX_SAFE_INTEGER)=>Number.isSafeInteger(value)&&value>=min&&value<=max;
export function originMetadata(input){
  check(input&&typeof input==='object'&&!Array.isArray(input)&&Object.keys(input).every(key=>fields.includes(key)));
  const value={...input,parent_id:input.parent_id??null,label:input.label??null,conflict:input.conflict===undefined?false:input.conflict};
  check(value.schema_version===1&&[value.id,value.game_id,value.device_id].every(id=>typeof id==='string'&&UUID.test(id))&&(value.parent_id===null||typeof value.parent_id==='string'&&UUID.test(value.parent_id))&&value.parent_id!==value.id);
  check([value.game_content_hash,value.core_config_hash,value.payload_hash].every(hash=>typeof hash==='string'&&HASH.test(hash)));
  check(['in_game','emulator_state'].includes(value.kind)&&integer(value.slot,0,65535)&&integer(value.generation,1)&&integer(value.payload_bytes,1,256*1024*1024)&&integer(value.created_at_ms,0));
  check(typeof value.core_id==='string'&&/^[A-Za-z0-9_-]{1,128}$/.test(value.core_id)&&text(value.core_version,256)&&validCoreVersion(value.core_version)&&text(value.state_format_version,512)&&text(value.platform,128)&&(value.label===null||text(value.label,256,true))&&typeof value.conflict==='boolean');
  check(Object.keys(value).length===fields.length&&encode(JSON.stringify(value)).length<=64*1024);return value;
}
export function originMetadataHash(input){
  const metadata=originMetadata(input),ordered=Object.fromEntries(Object.keys(metadata).sort().map(key=>[key,metadata[key]]));
  return new Sha256().update(encode('OmniPlay save origin v1\0')).update(encode(JSON.stringify(ordered))).hex();
}
export function sameSavePayload(a,b){return['game_content_hash','kind','slot','core_id','core_version','state_format_version','platform','core_config_hash','payload_hash','payload_bytes'].every(key=>a[key]===b[key]);}
export function sameSaveProjection(a,b){return a.id===b.id&&sameSavePayload(a,b)&&['created_at_ms','device_id','label'].every(key=>a[key]===b[key]);}
export function makeSaveOrigin(kind,input,importedFrom=null){
  const metadata=originMetadata(input);check(['captured','authenticated_remote','imported'].includes(kind));
  const imported=importedFrom===null?null:originMetadata(importedFrom);check((kind==='imported')===(imported!==null));
  if(imported)check(imported.id!==metadata.id&&sameSavePayload(imported,metadata));
  return{schema_version:1,save_id:metadata.id,kind,canonical_metadata:metadata,canonical_hash:originMetadataHash(metadata),imported_from:imported,imported_from_hash:imported?originMetadataHash(imported):null,missing_parent:false};
}
export function checkedSaveOrigin(input,projection){
  const keys=['schema_version','save_id','kind','canonical_metadata','canonical_hash','imported_from','imported_from_hash','missing_parent'];
  check(input&&typeof input==='object'&&!Array.isArray(input)&&Object.keys(input).length===keys.length&&Object.keys(input).every(key=>keys.includes(key))&&input.schema_version===1&&input.save_id===projection.id&&typeof input.missing_parent==='boolean');
  const local=originMetadata(projection);
  if(input.kind==='legacy_projection'){check(input.canonical_metadata===null&&input.canonical_hash===null&&input.imported_from===null&&input.imported_from_hash===null&&!input.missing_parent);return{...input};}
  check(['captured','authenticated_remote','imported'].includes(input.kind));
  const canonical=originMetadata(input.canonical_metadata);check(sameSaveProjection(local,canonical)&&originMetadataHash(canonical)===input.canonical_hash);
  check(!input.missing_parent||canonical.parent_id!==null);
  const imported=input.imported_from===null?null:originMetadata(input.imported_from);check((input.kind==='imported')===(imported!==null));
  if(imported)check(imported.id!==canonical.id&&sameSavePayload(local,imported)&&originMetadataHash(imported)===input.imported_from_hash);else check(input.imported_from_hash===null);
  return{...input,canonical_metadata:canonical,imported_from:imported};
}
