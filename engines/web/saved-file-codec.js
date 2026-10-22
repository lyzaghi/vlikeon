export const FILE_LIMITS=Object.freeze({bytes:256*1024*1024,entries:8192,files:4096,path:512,component:255,depth:8});
const encoder=new TextEncoder(),decoder=new TextDecoder('utf-8',{fatal:true,ignoreBOM:true});
function invalid(){throw new Error('The persistent-file save violates its bounded format.');}
export function filePath(value){
  if(typeof value!=='string'||!value||encoder.encode(value).length>FILE_LIMITS.path)invalid();const parts=value.split('/');if(parts.length>FILE_LIMITS.depth)invalid();
  for(const part of parts){
    if(!part||part==='.'||part==='..'||part.endsWith('.')||part.endsWith(' ')||encoder.encode(part).length>FILE_LIMITS.component||/[\u0000-\u001f\u007f-\u009f<>:\\|?*"]/.test(part))invalid();
    for(let i=0;i<part.length;i++){const code=part.charCodeAt(i);if(code>=0xd800&&code<=0xdfff){if(code>0xdbff||i+1>=part.length||part.charCodeAt(++i)<0xdc00||part.charCodeAt(i)>0xdfff)invalid();}}
    const stem=part.normalize('NFC').toUpperCase().split('.')[0];if(/^(CON|PRN|AUX|NUL|CONIN\$|CONOUT\$|CLOCK\$|COM[0-9¹²³]|LPT[0-9¹²³])$/.test(stem))invalid();
  }
  return parts;
}
export function fileAlias(value){filePath(value);return value.normalize('NFC').toUpperCase();}
function validate(entries){
  if(!Array.isArray(entries)||entries.length>FILE_LIMITS.entries)invalid();const explicit=new Map(),directories=new Set(),spellings=new Map();let files=0,total=12;
  for(const entry of entries){
    if(!entry||typeof entry.name!=='string'||typeof entry.directory!=='boolean'||!Number.isSafeInteger(entry.size)||entry.size<0||entry.directory&&entry.size!==0||!entry.directory&&typeof entry.read!=='function')invalid();
    const alias=fileAlias(entry.name);if(explicit.has(alias))invalid();explicit.set(alias,entry.directory);
    const parts=filePath(entry.name);for(let index=1;index<=parts.length;index++){const spelling=parts.slice(0,index).join('/'),key=fileAlias(spelling);if(spellings.has(key)&&spellings.get(key)!==spelling)invalid();spellings.set(key,spelling);}
    if(entry.directory)directories.add(alias);else if(++files>FILE_LIMITS.files)invalid();
    for(let at=alias.indexOf('/');at>=0;at=alias.indexOf('/',at+1))directories.add(alias.slice(0,at));
    if(directories.size+files>FILE_LIMITS.entries)invalid();total+=12+encoder.encode(entry.name).length+entry.size;if(!Number.isSafeInteger(total)||total>FILE_LIMITS.bytes)invalid();
  }
  for(const alias of explicit.keys())for(let at=alias.indexOf('/');at>=0;at=alias.indexOf('/',at+1))if(explicit.get(alias.slice(0,at))===false)invalid();
  let materialized=total;for(const alias of directories)if(!explicit.has(alias)){materialized+=12+encoder.encode(spellings.get(alias)).length;if(materialized>FILE_LIMITS.bytes)invalid();}return total;
}
export function decodeFiles(buffer){
  const bytes=buffer instanceof ArrayBuffer?new Uint8Array(buffer):buffer;if(!(bytes instanceof Uint8Array)||bytes.byteLength<12||bytes.byteLength>FILE_LIMITS.bytes)invalid();
  const magic=decoder.decode(bytes.subarray(0,8)),legacy=magic==='OPSF0001';if(!legacy&&magic!=='OPSF0002')invalid();
  const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),count=view.getUint32(8,true);if(count>(legacy?FILE_LIMITS.files:FILE_LIMITS.entries))invalid();let offset=12;const entries=[];
  for(let index=0;index<count;index++){
    const header=legacy?8:12;if(offset>bytes.length-header)invalid();const kind=legacy?0:view.getUint32(offset,true),nameLength=view.getUint32(offset+(legacy?0:4),true),size=view.getUint32(offset+(legacy?4:8),true);offset+=header;
    if(kind>1||kind===1&&size!==0||nameLength<1||nameLength>FILE_LIMITS.path||offset>bytes.length-nameLength||offset+nameLength>bytes.length-size)invalid();
    const name=decoder.decode(bytes.subarray(offset,offset+nameLength));offset+=nameLength;const payload=bytes.subarray(offset,offset+size);offset+=size;
    entries.push({name,directory:kind===1,size,read(output,at){const count=Math.min(output.byteLength,Math.max(0,payload.length-at));output.set(payload.subarray(at,at+count));return count;}});
  }
  if(offset!==bytes.length)invalid();validate(entries);return entries;
}
export function encodeFiles(entries){
  const total=validate(entries),ordered=entries.map(entry=>({entry,name:encoder.encode(entry.name)}));ordered.sort((a,b)=>{const length=Math.min(a.name.length,b.name.length);for(let i=0;i<length;i++)if(a.name[i]!==b.name[i])return a.name[i]-b.name[i];return a.name.length-b.name.length;});
  const output=new Uint8Array(total),view=new DataView(output.buffer);output.set(encoder.encode('OPSF0002'));view.setUint32(8,ordered.length,true);let offset=12;
  for(const{entry,name}of ordered){view.setUint32(offset,entry.directory?1:0,true);view.setUint32(offset+4,name.length,true);view.setUint32(offset+8,entry.size,true);offset+=12;output.set(name,offset);offset+=name.length;
    let copied=0;while(copied<entry.size){const part=output.subarray(offset+copied,offset+Math.min(entry.size,copied+65536)),count=entry.read(part,copied);if(!Number.isInteger(count)||count<=0||count>part.length)invalid();copied+=count;}offset+=entry.size;
  }
  return output.buffer;
}
