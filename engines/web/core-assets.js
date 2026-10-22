import{Sha256}from'./sha256.js';
import{mountMedia}from'./media-filesystem.js';
const HASH=/^[a-f0-9]{64}$/,PATH=/^[A-Za-z0-9_-][A-Za-z0-9_.-]*$/;
export function coreAssetPolicy(core){
  if(core?.system_assets===undefined){if(core?.system_assets_protocol!==undefined)throw new Error('System assets need an explicit inventory.');return[];}
  if(core.system_assets_protocol!==1||!Array.isArray(core.system_assets)||!core.system_assets.length||core.system_assets.length>32||!Array.isArray(core.firmware??[])||(core.firmware??[]).length)
    throw new Error('The installed core system-asset contract is unsupported.');
  const names=new Set(),resources=new Set(),spellings=new Map();let total=0;
  return core.system_assets.map(item=>{
    if(!item||Object.keys(item).length!==4||!['path','resource','bytes','sha256'].every(key=>Object.hasOwn(item,key))||typeof item.path!=='string'||item.path.length>240||
        item.path.split('/').length>4||item.path.split('/').some(part=>!PATH.test(part)||part==='.'||part==='..'||part.endsWith('.')||/^(?:CON|PRN|AUX|NUL|COM[0-9]|LPT[0-9])(?:\.|$)/i.test(part))||
        typeof item.resource!=='string'||item.resource.length>512||!item.resource.startsWith(core.module.split('/').slice(0,-1).join('/')+'/assets/')||
        item.resource.split('/').some(part=>!PATH.test(part)||part==='.'||part==='..')||!Number.isSafeInteger(item.bytes)||item.bytes<1||item.bytes>32*1024*1024||!HASH.test(item.sha256))
      throw new Error('The core system-asset inventory is invalid.');
    const alias=item.path.toLowerCase();if(names.has(alias)||resources.has(item.resource))throw new Error('Core system assets have duplicate paths.');
    const parts=item.path.split('/');for(let count=1;count<=parts.length;count++){const spelling=parts.slice(0,count).join('/'),key=spelling.toLowerCase();if(spellings.has(key)&&spellings.get(key)!==spelling)throw new Error('System asset directory spellings collide.');spellings.set(key,spelling);}
    for(const other of names)if(alias.startsWith(other+'/')||other.startsWith(alias+'/'))throw new Error('Core system assets contain a file/directory collision.');
    names.add(alias);resources.add(item.resource);total+=item.bytes;if(total>64*1024*1024)throw new Error('Core system assets exceed their byte budget.');return{...item};
  });
}
function cancelled(signal){if(signal?.aborted)throw new DOMException('System asset loading cancelled','AbortError');}
export async function loadCoreAssets(core,{signal}={}){
  const policy=coreAssetPolicy(core);if(!policy.length)return null;const files=[];
  for(const item of policy){
    cancelled(signal);const url=new URL(item.resource,location.href);if(url.origin!==location.origin||url.search||url.hash)throw new Error('System assets must be bundled with the application.');
    const response=await fetch(url,{signal,credentials:'same-origin',redirect:'error'});if(!response.ok||!response.body)throw new Error('A bundled core system asset is missing.');
    const reader=response.body.getReader(),output=new Uint8Array(item.bytes),hash=new Sha256();let offset=0;
    try{
      while(true){cancelled(signal);const part=await reader.read();if(part.done)break;if(!(part.value instanceof Uint8Array)||part.value.length>output.length-offset)throw new Error('A bundled system asset exceeds its declared length.');output.set(part.value,offset);hash.update(part.value);offset+=part.value.length;}
      cancelled(signal);if(offset!==output.length||hash.hex()!==item.sha256)throw new Error('A bundled system asset failed its integrity check.');
    }catch(error){try{await reader.cancel();}catch{}throw error;}finally{reader.releaseLock();}
    files.push({...item,buffer:output.buffer});
  }
  return{protocol:1,files,identity:policy.map(({path,bytes,sha256})=>({path,bytes,sha256}))};
}
export async function admitCoreAssets(params){
  if(!params||Object.keys(params).length!==2||params.protocol!==1||!Array.isArray(params.files))throw new Error('Core system assets are invalid.');
  const policy=coreAssetPolicy({module:'cores/core/core.js',system_assets_protocol:1,system_assets:params.files.map(({buffer,...item})=>({...item,resource:'cores/core/assets/'+item.path}))});
  for(let index=0;index<policy.length;index++){
    const item=params.files[index];if(Object.keys(item).length!==5||!(item.buffer instanceof ArrayBuffer)||item.buffer.byteLength!==item.bytes)throw new Error('Core system asset bytes are invalid.');
    const hash=new Sha256(),bytes=new Uint8Array(item.buffer);for(let at=0;at<bytes.length;at+=65536)hash.update(bytes.subarray(at,at+65536));if(hash.hex()!==item.sha256)throw new Error('Core system asset integrity changed in transfer.');
  }
  return params.files;
}
export async function mountCoreAssets(core,params){
  const files=await admitCoreAssets(params),records=new Map(files.map(file=>{
    const bytes=new Uint8Array(file.buffer);return[file.path,{entry:{byte_length:bytes.length},handle:{read(output,{at}){if(!Number.isSafeInteger(at)||at<0)throw new Error('System asset read offset is invalid.');const count=Math.min(output.byteLength,Math.max(0,bytes.length-at));output.set(bytes.subarray(at,at+count));return count;}}}];
  }));return mountMedia(core,records,'/system');
}
