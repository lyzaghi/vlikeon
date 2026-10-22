const encoder=new TextEncoder();
const identifier=value=>typeof value==='string'&&value.length>0&&value.length<=128&&!/[^A-Za-z0-9_-]/.test(value)&&!['__proto__','constructor','prototype'].includes(value);
const exposable=value=>!['jit','dynarec','cpucore','cpu_core','fast_memory','backend','renderer','render_mode','rdp-plugin','rsp-plugin','thread','wlan','network','server','sdcard','firmware','bios_path','sysfile','cheat','directory','_path'].some(part=>value.toLowerCase().includes(part));
const optionValue=value=>typeof value==='string'&&value.length>0&&value.length<=128&&!/[^\x20-\x7e]/.test(value);
const label=(value,max)=>typeof value==='string'&&value.length>0&&encoder.encode(value).length<=max&&!/[\u0000-\u001f\u007f-\u009f]/.test(value);
function invalid(message='Core options require a declared choice for every key.'){throw new Error(message);}
function object(value,fields){if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==fields.length||fields.some(key=>!Object.hasOwn(value,key)))invalid();}
export function coreOptionRuntime(value){if(!identifier(value))invalid('The core option runtime identity is invalid.');return value;}
export const defaultCoreOptions=()=>({schema_version:1,values:{}});
export function validateCoreOptions(value){
  object(value,['schema_version','values']);if(value.schema_version!==1||!value.values||typeof value.values!=='object'||Array.isArray(value.values)||Object.keys(value.values).length>64)invalid();
  const entries=Object.entries(value.values);if(entries.some(([key,v])=>!identifier(key)||!optionValue(v)))invalid();entries.sort(([a],[b])=>a<b?-1:a>b?1:0);
  const result={schema_version:1,values:Object.fromEntries(entries)};if(encoder.encode(JSON.stringify(result)).length>16384)invalid('Core options exceed their storage limit.');return result;
}
export function validateCoreOptionPolicy(value){
  object(value,['schema_version','definitions']);if(value.schema_version!==1||!Array.isArray(value.definitions)||value.definitions.length>32||encoder.encode(JSON.stringify(value)).length>65536)invalid('The installed core option policy exceeds its limit.');
  const keys=new Set();let count=0;const definitions=[];
  for(const definition of value.definitions){
    object(definition,['key','label_vi','label_en','description_vi','description_en','default_value','choices']);
    if(!identifier(definition.key)||!exposable(definition.key)||keys.has(definition.key)||!label(definition.label_vi,160)||!label(definition.label_en,160)||!label(definition.description_vi,512)||!label(definition.description_en,512)||!Array.isArray(definition.choices)||!definition.choices.length||definition.choices.length>32)invalid();
    keys.add(definition.key);count+=definition.choices.length;if(count>256)invalid();const values=new Set(),choices=[];
    for(const choice of definition.choices){object(choice,['value','label_vi','label_en']);if(!optionValue(choice.value)||values.has(choice.value)||!label(choice.label_vi,160)||!label(choice.label_en,160))invalid();values.add(choice.value);choices.push({...choice});}
    if(!values.has(definition.default_value))invalid();definitions.push({...definition,choices});
  }
  return{schema_version:1,definitions};
}
export function webCoreOptionPolicy(core){
  if(core?.core_options_protocol===undefined){if(core&&Object.hasOwn(core,'core_option_definitions'))invalid('Core options need an explicit installed protocol.');return{schema_version:1,definitions:[]};}
  if(core.core_options_protocol!==1)invalid('The installed core option protocol is unsupported.');const policy=validateCoreOptionPolicy({schema_version:1,definitions:core.core_option_definitions});
  if(policy.definitions.some(d=>d.key==='beetle_psx_enable_memcard1')&&(!core.capabilities?.includes('save_files')||core.saved_files_protocol!==1))invalid('A selectable second card requires persistent-file saves.');return policy;
}
export function resolveCoreOptions(policy,profile){
  policy=validateCoreOptionPolicy(policy);profile=validateCoreOptions(profile);const byKey=new Map(policy.definitions.map(item=>[item.key,item]));
  for(const[key,value]of Object.entries(profile.values)){const definition=byKey.get(key);if(!definition||!definition.choices.some(choice=>choice.value===value))invalid('Saved core options no longer match this installed runtime. Review or reset its game options.');}
  return Object.fromEntries([...byKey].sort(([a],[b])=>a<b?-1:a>b?1:0).map(([key,item])=>[key,profile.values[key]??item.default_value]));
}
