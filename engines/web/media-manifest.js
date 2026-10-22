const MAX_BYTES=4*1024*1024*1024;
const HASH=/^[a-f0-9]{64}$/;
const EXTENSIONS=['nes','gb','gbc','sfc','gba','gen','bin','z64','v64','n64','nds','iso','chd','cue','m3u','pbp','cso','wav'];
const ROM_EXTENSIONS={nes:'nes',gb:'gb',gbc:'gbc',snes:'sfc',gba:'gba',mega_drive:'gen',ps1:'bin',n64:'z64',nds:'nds',psp:'iso'};
function invalid(){throw new Error('The game media manifest violates its content policy.');}
function object(value,required,optional=[]){if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).some(key=>!required.includes(key)&&!optional.includes(key))||required.some(key=>!Object.hasOwn(value,key)))invalid();}
export function mediaName(value){if(typeof value!=='string'||value.length>64||!/^(game|track-[0-9]{4}|disc-[0-9]{2})\.[a-z0-9]+$/.test(value))return false;return EXTENSIONS.includes(value.slice(value.lastIndexOf('.')+1));}
export function mediaExtension(format,system){
  if(format==='raw_rom'){if(!Object.hasOwn(ROM_EXTENSIONS,system))invalid();return ROM_EXTENSIONS[system];}
  if(['cue','m3u'].includes(format)&&system==='ps1')return format;
  if(['chd','iso','pbp'].includes(format)&&['ps1','psp'].includes(system))return format;
  if(format==='cso'&&system==='psp')return format;
  if(system==='n64'&&['n64_z64','n64_v64','n64_n64'].includes(format))return{n64_z64:'z64',n64_v64:'v64',n64_n64:'n64'}[format];invalid();
}
export function normalizeMedia(value,system,total){
  object(value,['schema_version','format','entry_name','assets'],['disc_entries']);
  if(typeof system!=='string'||typeof value.format!=='string')invalid();
  if(!Number.isSafeInteger(total)||total<=0||total>MAX_BYTES||value.schema_version!==1||value.entry_name!==`game.${mediaExtension(value.format,system)}`||!Array.isArray(value.assets)||!value.assets.length||value.assets.length>128)invalid();
  let bytes=0,previous='';const names=new Set(),assets=[];
  for(const asset of value.assets){object(asset,['name','content_hash','byte_length']);if(!mediaName(asset.name)||asset.name<=previous||typeof asset.content_hash!=='string'||!HASH.test(asset.content_hash)||!Number.isSafeInteger(asset.byte_length)||asset.byte_length<=0||asset.byte_length>MAX_BYTES)invalid();bytes+=asset.byte_length;if(!Number.isSafeInteger(bytes)||bytes>MAX_BYTES)invalid();previous=asset.name;names.add(asset.name);assets.push({name:asset.name,content_hash:asset.content_hash,byte_length:asset.byte_length});}
  const discs=Object.hasOwn(value,'disc_entries')?value.disc_entries:[];if(!Array.isArray(discs)||discs.length>8||discs.some(name=>typeof name!=='string'||!names.has(name))||!names.has(value.entry_name)||bytes!==total||['cue','m3u'].includes(value.format)&&assets.length<2||value.format==='m3u'&&!discs.length)invalid();
  // Property order is the Rust GameMedia/MediaAsset Serialize field order.
  return{schema_version:1,format:value.format,entry_name:value.entry_name,assets,disc_entries:[...discs]};
}
export async function mediaIdentity(media){
  if(media.assets.length===1&&!['cue','m3u'].includes(media.format))return media.assets[0].content_hash;
  const encoded=new TextEncoder().encode('omniplay.media.v1\0'+JSON.stringify(media));
  return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',encoded)),value=>value.toString(16).padStart(2,'0')).join('');
}
export async function gameMedia(game){
  if(!game||!HASH.test(game.content_hash)||!Number.isSafeInteger(game.byte_length)||game.byte_length<=0||game.byte_length>MAX_BYTES)invalid();
  const legacy=game.media==null,extension=mediaExtension(game.media?.format??'raw_rom',game.system),value=legacy?{schema_version:1,format:'raw_rom',entry_name:`game.${extension}`,assets:[{name:`game.${extension}`,content_hash:game.content_hash,byte_length:game.byte_length}],disc_entries:[]}:game.media;
  const media=normalizeMedia(value,game.system,game.byte_length);if(await mediaIdentity(media)!==game.content_hash)throw new Error('The media identity does not match this game.');
  return{media,legacy,extension,content_hash:game.content_hash,total:game.byte_length,system:game.system,staged:media.assets.length>1||game.byte_length>256*1024*1024||media.format!=='raw_rom'};
}
export function coreSupportsMedia(core,plan){
  if(!core||!Array.isArray(core.systems)||!core.systems.includes(plan.system))return false;
  if(plan.media.format==='m3u'&&(!core.capabilities?.includes('disc_control')||core.disc_state_protocol!==1))return false;
  if(core.content_formats===undefined)return!plan.staged&&plan.media.format==='raw_rom';
  return Array.isArray(core.content_formats)&&core.content_formats.includes(plan.media.format)&&(!plan.staged||core.media_filesystem_protocol===1);
}
