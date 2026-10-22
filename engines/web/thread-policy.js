const FIELDS=['schema_version','api','maximum_workers','maximum_memory_bytes','worker_module','worker_type'];
const PATH=/^cores\/[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+\.js$/;
export function validateThreadPolicy(value){
  if(value===null)return null;
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==FIELDS.length||FIELDS.some(key=>!Object.hasOwn(value,key))||
      value.schema_version!==1||value.api!=='emscripten_pthreads_ui_v1'||value.worker_type!=='module'||
      !Number.isInteger(value.maximum_workers)||value.maximum_workers<1||value.maximum_workers>16||
      !Number.isSafeInteger(value.maximum_memory_bytes)||value.maximum_memory_bytes<64*1024*1024||value.maximum_memory_bytes>1024*1024*1024||value.maximum_memory_bytes%65536||
      typeof value.worker_module!=='string'||value.worker_module.length>512||!PATH.test(value.worker_module)||value.worker_module.split('/').some(part=>part==='.'||part==='..'))
    throw new Error('The installed emulator thread policy is unsupported.');
  return Object.freeze({...value});
}
export function coreThreadPolicy(core){
  if(core?.wasm_threads!==true){
    if(core?.wasm_threads!==undefined&&core.wasm_threads!==false||core?.thread_pool!==undefined||core?.thread_callbacks_protocol!==undefined||core?.pthread_worker_sha256!==undefined||core?.pthread_worker_bytes!==undefined)
      throw new Error('Thread resources require their explicit execution policy.');
    return null;
  }
  const policy=validateThreadPolicy(core.thread_pool);
  if(!policy||core.thread_callbacks_protocol!==1||core.libretro_result_protocol!==2||core.hardware_video!==undefined||core.hardware_video_protocol!==undefined||
      typeof core.module!=='string'||core.module.split('/').slice(0,-1).join('/')!==policy.worker_module.split('/').slice(0,-1).join('/')||
      !/^[a-f0-9]{64}$/.test(core.pthread_worker_sha256)||!Number.isSafeInteger(core.pthread_worker_bytes)||core.pthread_worker_bytes<1||core.pthread_worker_bytes>32*1024*1024)
    throw new Error('The installed emulator lacks its typed thread callback and resource contract.');
  return policy;
}
export function threadsAvailable(){
  return globalThis.crossOriginIsolated===true&&typeof SharedArrayBuffer==='function'&&typeof Atomics==='object'&&typeof Worker==='function'&&typeof globalThis.WebAssembly?.Memory==='function';
}
export function threadWorkerUrl(policy,moduleUrl){
  policy=validateThreadPolicy(policy);if(!policy)return null;
  const module=new URL(moduleUrl,globalThis.location.href),url=new URL(policy.worker_module.split('/').at(-1),module);
  if(module.origin!==globalThis.location.origin||module.search||module.hash||url.origin!==module.origin||!module.pathname.includes('/cores/')||
      module.pathname.split('/').at(-2)!==policy.worker_module.split('/')[1])throw new Error('Thread code must be bundled beside its emulator module.');
  return url.href;
}
