export const MD_PORTABLE_FORMAT='ares-md-canonical-files-v1';
const PIN='9408cb43d4948fc3ea6e152a307a34348df3fe04',VERSION='0.3.0+ares.9408cb43d494.omweb1';
const LIMIT=2*1024*1024+128,EEPROM=[0,128,128,256,512,1024,2048,4096,8192,8192,16384,32768,65536];
const encoder=new TextEncoder(),names=['save.ram','save.eeprom'],encoded=names.map(name=>encoder.encode(name));
function invalid(){throw Object.assign(new Error('Progress does not match this game’s supported persistent save format.'),{code:'incompatible_save'});}
function bytes(value){if(value instanceof ArrayBuffer)return new Uint8Array(value);if(value instanceof Uint8Array)return value;invalid();}
function magic(data,text){return data.length>=text.length&&Array.from(text).every((value,index)=>data[index]===value.charCodeAt(0));}
function number(data,at){if(!Number.isInteger(at)||at<0||at>data.length-4)invalid();return new DataView(data.buffer,data.byteOffset+at,4).getUint32(0,true);}
export function webPortableFilesPolicy(core){
  if(core?.portable_in_game_format===undefined&&core?.portable_files_protocol===undefined)return null;
  if(core?.portable_in_game_format!==MD_PORTABLE_FORMAT||core.portable_files_protocol!==1||core.in_game_format_version!==MD_PORTABLE_FORMAT||core.id!=='ares-md-web'||core.core_version!==VERSION||core.runtime_version_string!==VERSION||core.source_revision!==PIN||!Array.isArray(core.systems)||core.systems.length!==1||core.systems[0]!=='mega_drive'||core.saved_files_protocol!==1||core.saved_files_io!=='single_threaded_barrier_v1'||core.wasm_threads!==false||core.libretro_result_protocol!==1||!core.capabilities?.includes('save_files'))throw new Error('The installed emulator has an unsupported portable save declaration.');
  return{protocol:1,profile:'ares_md_v1',format:MD_PORTABLE_FORMAT};
}
export function validatePortableFilesRequest(value){
  if(value===null)return null;
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==3||!['protocol','profile','format'].every(key=>Object.hasOwn(value,key))||value.protocol!==1||value.profile!=='ares_md_v1'||value.format!==MD_PORTABLE_FORMAT)throw new Error('The portable progress request is invalid.');
  return{protocol:1,profile:'ares_md_v1',format:MD_PORTABLE_FORMAT};
}
export function portableMdFiles(payload){
  let data=bytes(payload);if(data.length>LIMIT+32)invalid();
  if(magic(data,'OPWH0001')){
    if(data.length<44||number(data,8)!==16||number(data,12)+32!==data.length||!magic(data.subarray(16),'OPRM0001')||number(data,24)!==0||number(data,28)!==0)invalid();data=data.subarray(32);
  }
  if(data.length<12||data.length>LIMIT||!magic(data,'OPSF0002'))invalid();return data;
}
export class MdPortableFiles{
  #core;#slot=0;#contract=null;
  constructor(core,policy){
    if(validatePortableFilesRequest(policy)===null||typeof core._om_md_describe_save!=='function')throw new Error('The installed emulator is missing its portable save contract.');
    this.#core=core;this.#slot=core._malloc(24);if(!this.#slot||this.#slot%4||this.#slot+24>core.HEAPU8.length)throw new Error('The portable save contract could not be allocated.');
  }
  #layoutFailure(){throw Object.assign(new Error('The emulator did not provide a stable persistent save layout.'),{unrecoverable:true});}
  contract(){
    if(!this.#slot||this.#slot+24>this.#core.HEAPU8.length)this.#layoutFailure();
    this.#core.HEAPU32.fill(0,this.#slot>>>2,(this.#slot+24)>>>2);this.#core._om_md_describe_save(this.#slot);
    if(this.#slot+24>this.#core.HEAPU8.length)this.#layoutFailure();const w=Array.from(this.#core.HEAPU32.subarray(this.#slot>>>2,(this.#slot+24)>>>2));
    if(w[0]!==0x3143444d||w[1]!==1||w[2]>1024*1024||w[3]>1024*1024||w[4]>3||w[5]>=EEPROM.length||(w[2]===0)!==(w[4]===0)||w[4]===1&&(w[2]<2||w[2]%2)||EEPROM[w[5]]!==w[3]||this.#core._retro_get_memory_size(0)!==0||this.#core._retro_get_memory_size(1)!==0)this.#layoutFailure();
    if(this.#contract&&this.#contract.some((value,index)=>value!==w[index]))throw Object.assign(new Error('The emulator changed its persistent save layout.'),{unrecoverable:true});
    this.#contract=w;return w;
  }
  admitEntries(entries){
    const contract=this.contract(),seen=new Set();if(!Array.isArray(entries)||entries.length>2)invalid();
    for(const entry of entries){const index=names.indexOf(entry.name);if(index<0||entry.directory||seen.has(index)||!contract[index+2]||entry.size!==contract[index+2])invalid();seen.add(index);}
    if(names.some((name,index)=>contract[index+2]&&!seen.has(index)))invalid();
  }
  available(){const contract=this.contract();return contract[2]>0||contract[3]>0;}
  validate(archive){
    const data=bytes(archive),contract=this.contract();if(data.length<12||data.length>LIMIT||!magic(data,'OPSF0002')||number(data,8)>2)invalid();
    let at=12;const seen=new Set();
    for(let index=0;index<number(data,8);index++){
      const kind=number(data,at),length=number(data,at+4),size=number(data,at+8);at+=12;if(kind||length>11||at>data.length-length)invalid();
      const name=encoded.findIndex(value=>value.length===length&&value.every((byte,index)=>data[at+index]===byte));
      if(name<0||seen.has(name)||!contract[name+2]||size!==contract[name+2])invalid();seen.add(name);at+=length;if(size>data.length-at)invalid();at+=size;
    }
    if(at!==data.length||names.some((name,index)=>contract[index+2]&&!seen.has(index)))invalid();return data;
  }
  dispose(){if(this.#slot)this.#core._free(this.#slot);this.#slot=0;}
}
