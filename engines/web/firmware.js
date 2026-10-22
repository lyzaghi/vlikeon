import{mountMedia}from'./media-filesystem.js';
const ROLES={snes:{snes_spc_ipl:[64]},mega_drive:{mega_drive_svp:[2048]},ps1:{ps1_bios:[524288],ps1_bios_jp:[524288],ps1_bios_eu:[524288]},nds:{nds_bios7:[16384],nds_bios9:[4096],nds_firmware:[131072,262144,524288]}};
const HASH=/^[a-f0-9]{64}$/;
export function firmwareRoleSizes(system,role){if(typeof system!=='string'||typeof role!=='string'||!Object.hasOwn(ROLES,system)||!Object.hasOwn(ROLES[system],role))throw new Error('This firmware role is unsupported.');return[...ROLES[system][role]];}
function invalid(){throw new Error('The installed core has an invalid firmware policy.');}
export function firmwareRequirements(value,system){
  if(value===undefined)return[];if(!Array.isArray(value)||value.length>8)invalid();const names=new Set();
  return value.map(item=>{
    if(!item||typeof item!=='object'||Array.isArray(item)||Object.keys(item).some(key=>!['role','filename','allowed_bytes','required_group','optional'].includes(key))||typeof item.role!=='string'||!Object.hasOwn(ROLES[system]??{},item.role)||typeof item.filename!=='string'||item.filename.length>64||!/^[A-Za-z0-9_-][A-Za-z0-9_.-]*$/.test(item.filename)||item.filename.endsWith('.')||names.has(item.filename)||!Array.isArray(item.allowed_bytes)||!item.allowed_bytes.length||item.allowed_bytes.length>4||new Set(item.allowed_bytes).size!==item.allowed_bytes.length||item.allowed_bytes.some(size=>!ROLES[system][item.role].includes(size)))invalid();
    if(item.optional!==undefined&&typeof item.optional!=='boolean'||item.required_group!==undefined&&(typeof item.required_group!=='string'||!/^[a-z0-9_]{1,64}$/.test(item.required_group)||item.optional===true))invalid();names.add(item.filename);
    return{role:item.role,filename:item.filename,allowed_bytes:[...item.allowed_bytes],...(item.required_group===undefined?{}:{required_group:item.required_group}),...(item.optional===undefined?{}:{optional:item.optional})};
  });
}
export async function admitFirmware(params){
  if(!params||typeof params!=='object'||Array.isArray(params)||!Array.isArray(params.files))invalid();
  const policy=firmwareRequirements(params.requirements,params.system);if(params.files.length>policy.length)invalid();
  const seen=new Set(),groups=new Set(),files=[];let total=0;
  for(const file of params.files){
    if(!file||typeof file!=='object'||Array.isArray(file)||Object.keys(file).some(key=>!['role','filename','sha256','byte_length','bytes'].includes(key)))invalid();
    const expected=policy.find(item=>item.filename===file.filename&&item.role===file.role);
    if(!expected||seen.has(file.filename)||!(file.bytes instanceof ArrayBuffer)||file.byte_length!==file.bytes.byteLength||!expected.allowed_bytes.includes(file.byte_length)||typeof file.sha256!=='string'||!HASH.test(file.sha256))invalid();
    total+=file.byte_length;if(total>4*1024*1024)invalid();
    const hash=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',file.bytes)),value=>value.toString(16).padStart(2,'0')).join('');if(hash!==file.sha256)throw new Error('The selected firmware failed its worker integrity check.');
    seen.add(file.filename);if(expected.required_group)groups.add(expected.required_group);files.push(file);
  }
  if(policy.some(item=>!seen.has(item.filename)&&(item.required_group?!groups.has(item.required_group):item.optional!==true)))throw new Error('The selected core requires firmware from your local vault.');return files;
}
export async function mountFirmware(core,params){
  const files=await admitFirmware(params);if(!files.length)return null;
  const records=new Map(files.map(file=>{const bytes=new Uint8Array(file.bytes);return[file.filename,{entry:{byte_length:bytes.byteLength},handle:{read(output,{at}){if(!Number.isSafeInteger(at)||at<0)throw new Error('Firmware read offset is invalid.');const count=Math.min(output.byteLength,Math.max(0,bytes.length-at));output.set(bytes.subarray(at,at+count));return count;}}}];}));
  return mountMedia(core,records,'/system');
}
