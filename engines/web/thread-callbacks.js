const NAMES=['environment','video_refresh','audio_sample','audio_sample_batch','input_poll','input_state','rumble'];
export class ThreadCallbacks{
  #core;#fault=false;#closed=false;#maximum;#minimum;#pointers;
  constructor(core,policy,callbacks,minimumMemoryBytes){
    this.#core=core;this.#maximum=policy.maximum_memory_bytes;this.#minimum=minimumMemoryBytes;
    if(!Number.isSafeInteger(this.#minimum)||this.#minimum<65536||this.#minimum>this.#maximum)throw new Error('The emulator shared-memory floor is invalid.');
    if(typeof core._om_web_thread_abi!=='function'||typeof core._om_web_thread_callback_pointer!=='function'||core._om_web_thread_abi()!==1)
      throw new Error('The installed emulator is missing its typed thread frontend.');
    const pointers={},safe={};
    for(const[name,index]of NAMES.map((name,index)=>[name,index])){
      if(typeof callbacks[name]!=='function')throw new Error('The emulator thread callback is unavailable.');
      const pointer=core._om_web_thread_callback_pointer(index);
      if(!Number.isInteger(pointer)||pointer<1||pointer>0x7fffffff)throw new Error('The typed thread callback address is invalid.');
      pointers[name]=pointer;
      safe[name]=(...args)=>{
        if(this.#closed||this.#fault)return 0;
        try{this.check();return callbacks[name](...args);}catch{this.#fault=true;return 0;}
      };
    }
    this.#pointers=Object.freeze(pointers);core['omniplayCallbacksV1']=Object.freeze(safe);this.check();
  }
  get pointers(){return this.#pointers;}
  check(){
    if(this.#closed||this.#fault||!(this.#core.HEAPU8?.buffer instanceof SharedArrayBuffer)||this.#core.HEAPU8.byteLength<this.#minimum||this.#core.HEAPU8.byteLength>this.#maximum){
      this.#fault=true;throw Object.assign(new Error('The typed emulator thread frontend failed. Reopen the game before continuing.'),{unrecoverable:true});
    }
  }
  close(){if(this.#closed)return;this.#closed=true;this.#core['omniplayCallbacksV1']=null;}
}
