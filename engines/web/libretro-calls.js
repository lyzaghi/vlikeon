// Cooperative-fiber exports can unwind/rewind before an outer Wasm export
// returns to JavaScript. A void bridge writes its final result into a retained
// completion slot rather than relying on the initial unwind's return value.
import{coreThreadPolicy}from'./thread-policy.js';
export function libretroResultProtocol(core){
  const threads=coreThreadPolicy(core);
  if(core?.libretro_result_protocol===undefined)return 0;
  if(![1,2].includes(core.libretro_result_protocol)||(threads?core.libretro_result_protocol!==2:core.wasm_threads!==false))throw new Error('The installed emulator has an unsupported result protocol.');
  return core.libretro_result_protocol;
}
export class LibretroCalls{
  #core;#protocol;#slot=0;#busy=false;#poisoned=false;#guard;
  constructor(core,protocol,guard=()=>{}){
    if(![0,1,2].includes(protocol))throw new Error('The emulator result protocol is unsupported.');
    if(typeof guard!=='function')throw new Error('The emulator call guard is invalid.');
    this.#core=core;this.#protocol=protocol;this.#guard=guard;
    if(protocol){
      if(typeof core._om_web_libretro_call!=='function')throw new Error('The emulator is missing its declared result bridge.');
      this.#slot=core._malloc(8);if(!this.#slot||this.#slot%4||this.#slot+8>core.HEAPU8.length)throw new Error('The emulator result slot could not be allocated.');
    }
  }
  get protocol(){return this.#protocol;}
  get healthy(){return!this.#poisoned;}
  #fatal(){this.#poisoned=true;throw Object.assign(new Error('The emulator did not complete its cooperative call. Reopen the game before continuing.'),{unrecoverable:true});}
  #call(operation,first=0,second=0){
    if(this.#poisoned||this.#busy)this.#fatal();
    try{this.#guard();}catch(error){this.#poisoned=true;throw error;}
    const names=['','_retro_load_game','_retro_serialize_size','_retro_serialize','_retro_unserialize','_om_web_flush_save_io','_retro_run','_retro_unload_game','_retro_deinit','_retro_init','_retro_reset'];
    if(!this.#protocol||this.#protocol===1&&operation>=6){const fn=this.#core[names[operation]];if(typeof fn!=='function')throw new Error('The emulator call is unavailable.');const result=operation===2||operation>=5?fn():operation===1?fn(first):fn(first,second);try{this.#guard();}catch(error){this.#poisoned=true;throw error;}return result;}
    if(![first,second].every(value=>Number.isInteger(value)&&value>=0&&value<=0xffffffff)||!this.#slot||this.#slot+8>this.#core.HEAPU8.length)this.#fatal();
    this.#busy=true;
    try{
      this.#core.HEAPU32[this.#slot>>>2]=0;this.#core.HEAPU32[(this.#slot+4)>>>2]=0;
      this.#core._om_web_libretro_call(operation,first,second,this.#slot);
      // Reacquire heap views after any grow. A missing completion is terminal;
      // retain this slot until worker retirement, so a late write cannot target
      // memory that the frontend has freed for another allocation.
      if(this.#slot+8>this.#core.HEAPU8.length||this.#core.HEAPU32[(this.#slot+4)>>>2]!==1)this.#fatal();
      const result=this.#core.HEAPU32[this.#slot>>>2];if(operation!==2&&result>1)this.#fatal();this.#guard();return result;
    }catch(error){this.#poisoned=true;throw Object.assign(error instanceof Error?error:new Error('The cooperative emulator call failed.',{cause:error}),{unrecoverable:true});}
    finally{this.#busy=false;}
  }
  loadGame(pointer){return this.#call(1,pointer);}
  serializeSize(){return this.#call(2);}
  serialize(pointer,size){return this.#call(3,pointer,size);}
  unserialize(pointer,size){return this.#call(4,pointer,size);}
  flushSaveIo(){return this.#call(5);}
  #void(operation){const value=this.#call(operation);if(this.#protocol>=2&&value!==1)this.#fatal();}
  run(){this.#void(6);}
  unloadGame(){this.#void(7);}
  deinit(){this.#void(8);}
  init(){this.#void(9);}
  reset(){this.#void(10);}
  dispose(){if(this.#slot&&!this.#poisoned)this.#core._free(this.#slot);this.#slot=0;this.#poisoned=true;}
}
