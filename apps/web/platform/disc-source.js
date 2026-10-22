import{integer,utf8,requireValue,checkAbort,fail}from'./errors.js';
const controls=/[\u0000-\u001f\u007f-\u009f]/;
const ENTRIES=new Set(['cue','m3u','m3u8','chd','iso','pbp']);
export const DISC_LIMITS=Object.freeze({files:4096,directories:256,metadata:2*1024*1024,depth:8,path:2048,component:255,entries:64});
function spelling(value){
  requireValue(typeof value==='string'&&value.length>0&&!controls.test(value),'invalid_request','The selected folder contains an invalid filename.');
  for(let index=0;index<value.length;index++){const code=value.charCodeAt(index);if(code>=0xd800&&code<=0xdfff){requireValue(code<=0xdbff&&index+1<value.length,'invalid_request','A folder filename has invalid Unicode.');const next=value.charCodeAt(++index);requireValue(next>=0xdc00&&next<=0xdfff,'invalid_request','A folder filename has invalid Unicode.');}}
  return value;
}
function component(value){spelling(value);requireValue(value!=='.'&&value!=='..'&&!value.includes(':')&&!value.includes('\\')&&!value.endsWith('.')&&!value.endsWith(' ')&&utf8(value).length<=DISC_LIMITS.component,'permission_denied','A disc path has an ambiguous or unsupported component.');return value;}
export function discReference(base,value){
  spelling(value);const normalized=value.replaceAll('\\','/');requireValue(utf8(normalized).length<=DISC_LIMITS.path&&!normalized.startsWith('/')&&!normalized.includes(':'),'permission_denied','Disc references must stay inside the selected folder.');
  const parts=base?base.split('/'):[];
  for(const part of normalized.split('/')){
    if(part==='.')continue;if(part==='..'){requireValue(parts.length>0,'permission_denied','A disc reference escapes the selected folder.');parts.pop();}
    else parts.push(component(part));
  }
  requireValue(parts.length>0&&parts.length<=DISC_LIMITS.depth,'invalid_request','Disc reference nesting exceeds its bound.');const result=parts.join('/');requireValue(utf8(result).length<=DISC_LIMITS.path,'invalid_request','The disc reference is too long.');return result;
}
export const discExtension=value=>value.slice(value.lastIndexOf('.')+1).toLowerCase();
export function discFolderSelection(fileList,signal){
  checkAbort(signal);requireValue(fileList&&integer(fileList.length,0,DISC_LIMITS.files),'resource_limit','Choose a folder containing at most 4096 files.');requireValue(fileList.length>0,'not_found','The selected folder contains no files. Choose the folder with the disc entry point.');
  let root=null,metadataBytes=0;const files=new Map(),spellings=new Map(),directories=new Set(),entries=[];
  for(let index=0;index<fileList.length;index++){
    checkAbort(signal);const file=fileList[index];requireValue(file instanceof File&&typeof file.webkitRelativePath==='string'&&integer(file.size,0,Number.MAX_SAFE_INTEGER),'invalid_request','The browser did not supply a complete selected-folder file list.');
    const path=spelling(file.webkitRelativePath),parts=path.split('/');requireValue(parts.length>=2&&parts.length<=DISC_LIMITS.depth+1,'invalid_request','The selected folder exceeds its nesting limit.');for(const part of parts)component(part);
    if(root===null)root=parts[0];requireValue(parts[0]===root&&file.name===parts.at(-1),'permission_denied','Selected files do not share the same folder identity.');
    const relative=parts.slice(1).join('/');requireValue(utf8(relative).length<=DISC_LIMITS.path,'invalid_request','A selected disc path is too long.');
    metadataBytes+=utf8(path).length+utf8(file.name).length+64;requireValue(metadataBytes<=DISC_LIMITS.metadata,'resource_limit','Selected-folder metadata exceeds its limit.');
    // No guessed case-insensitive fallback: preserve exact names while refusing
    // aliases that different filesystems could resolve to conflicting entries.
    for(let depth=1;depth<parts.length;depth++){
      const name=parts.slice(1,depth+1).join('/'),alias=name.normalize('NFC').toUpperCase();requireValue(!spellings.has(alias)||spellings.get(alias)===name,'invalid_request','The folder contains ambiguous Unicode or case aliases.');spellings.set(alias,name);
      if(depth<parts.length-1)directories.add(name);
    }
    requireValue(directories.size<=DISC_LIMITS.directories&&!files.has(relative),'resource_limit','The selected folder has too many directories or duplicate paths.');files.set(relative,file);
    if(parts.length===2&&ENTRIES.has(discExtension(relative))){entries.push(relative);requireValue(entries.length<=DISC_LIMITS.entries,'resource_limit','Choose a folder with at most 64 disc entry points.');}
  }
  for(const directory of directories)requireValue(!files.has(directory),'invalid_request','A selected file shadows a referenced directory.');
  requireValue(entries.length>0,'not_found','Choose the folder that directly contains the CUE, M3U, CHD, ISO or PBP entry point.');
  entries.sort((a,b)=>{const aa=utf8(a),bb=utf8(b),length=Math.min(aa.length,bb.length);for(let at=0;at<length;at++)if(aa[at]!==bb[at])return aa[at]-bb[at];return aa.length-bb.length;});
  const suggested_title=Array.from(root).slice(0,120).join('');return{files,entries,suggested_title};
}
export function folderFile(scope,relative){
  requireValue(scope?.files instanceof Map&&discReference('',relative)===relative,'permission_denied','The disc file is outside its selected capability.');const file=scope.files.get(relative);if(!file)fail('not_found','A referenced disc file is absent from the selected folder.');requireValue(file instanceof Blob&&integer(file.size,1,4*1024**3),'resource_limit','A referenced disc file is empty or exceeds four GiB.');return file;
}
