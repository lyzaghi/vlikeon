const FIELDS=['schema_version','api','maximum_width','maximum_height'];
export function hardwareVideoPolicy(core){
  const value=core?.hardware_video;
  if(value===undefined){if(core?.hardware_video_protocol!==undefined)throw new Error('Hardware video needs an explicit target.');return null;}
  if(core.hardware_video_protocol!==1||core.wasm_threads!==false||core.libretro_result_protocol!==2)throw new Error('The installed hardware renderer has an unsupported execution protocol.');
  return validateHardwareVideo(value);
}
export function validateHardwareVideo(value){
  if(value===null)return null;
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==FIELDS.length||FIELDS.some(key=>!Object.hasOwn(value,key))||
    value.schema_version!==1||value.api!=='webgl2'||!Number.isInteger(value.maximum_width)||!Number.isInteger(value.maximum_height)||
    value.maximum_width<1||value.maximum_width>1920||value.maximum_height<1||value.maximum_height>1080||value.maximum_width*value.maximum_height>1920*1080)
    throw new Error('The installed hardware video target is outside its bounds.');
  return{...value};
}
export function hardwareVideoAvailable(){
  if(typeof OffscreenCanvas!=='function'||typeof HTMLCanvasElement==='undefined'||typeof HTMLCanvasElement.prototype.transferControlToOffscreen!=='function')return false;
  let context=null;
  try{context=new OffscreenCanvas(2,2).getContext('webgl2',{alpha:false,depth:false,stencil:false,antialias:false});return Boolean(context);}
  catch{return false;}
  finally{try{context?.getExtension('WEBGL_lose_context')?.loseContext();}catch{}}
}
export class HardwareVideo{
  #core;#policy;#slot=0;#registered=false;#active=false;#poisoned=false;#busy=false;#canvas;#loss;
  constructor(core,policy,canvas,onLoss,power){
    this.#core=core;this.#policy=validateHardwareVideo(policy);this.#canvas=canvas;
    for(const name of ['prepare','register','call','alive','callback_bytes'])if(typeof core['_om_web_hw_'+name]!=='function')throw new Error('The declared hardware video frontend is missing.');
    this.#slot=core._malloc(8);if(!this.#slot||this.#slot%4||this.#slot+8>core.HEAPU8.length)throw new Error('Hardware video completion storage is unavailable.');
    this.#loss=event=>{event.preventDefault();if(this.#poisoned)return;this.#poisoned=true;onLoss();};
    canvas.addEventListener('webglcontextlost',this.#loss);
    if(core._om_web_hw_prepare(policy.maximum_width,policy.maximum_height,power==='battery'?1:0)!==1)this.#fatal();
  }
  get healthy(){return!this.#poisoned;}
  #fatal(){this.#poisoned=true;throw Object.assign(new Error('The hardware video context is unavailable. Reopen the game before continuing.'),{unrecoverable:true});}
  #call(operation,first=0,second=0){
    if(this.#poisoned||this.#busy||!this.#slot||this.#slot+8>this.#core.HEAPU8.length)this.#fatal();
    this.#busy=true;
    try{
      this.#core.HEAPU32[this.#slot>>>2]=0;this.#core.HEAPU32[(this.#slot+4)>>>2]=0;
      this.#core._om_web_hw_call(operation,first,second,this.#slot);
      if(this.#slot+8>this.#core.HEAPU8.length||this.#core.HEAPU32[(this.#slot+4)>>>2]!==1||this.#core.HEAPU32[this.#slot>>>2]!==1)this.#fatal();
    }catch(error){this.#poisoned=true;throw Object.assign(error instanceof Error?error:new Error('Hardware video did not complete.',{cause:error}),{unrecoverable:true});}
    finally{this.#busy=false;}
  }
  register(pointer){
    if(this.#poisoned||this.#registered)return 0;
    const size=this.#core._om_web_hw_callback_bytes();
    if(!Number.isInteger(size)||size<32||size>128||!Number.isInteger(pointer)||pointer<=0||pointer%4||pointer>this.#core.HEAPU8.length-size)return 0;
    if(this.#core._om_web_hw_register(pointer)!==1)return 0;
    this.#registered=true;return 1;
  }
  activate(){if(!this.#registered||this.#active)this.#fatal();this.#call(1);this.#active=true;}
  beforeRun(){if(!this.#active||this.#poisoned||this.#core._om_web_hw_alive()!==1)this.#fatal();}
  present(width,height){
    if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width>this.#policy.maximum_width||height>this.#policy.maximum_height)this.#fatal();
    this.beforeRun();this.#call(2,width,height);
  }
  beforeGuestStop(){this.#call(3);this.#active=false;}
  afterGuestStop(){this.#call(4);this.#registered=false;}
  reinitialize(power){if(this.#registered||this.#active||this.#poisoned)this.#fatal();if(this.#core._om_web_hw_prepare(this.#policy.maximum_width,this.#policy.maximum_height,power==='battery'?1:0)!==1)this.#fatal();}
  detach(canCallGuest=true){this.#canvas?.removeEventListener('webglcontextlost',this.#loss);this.#canvas=null;if(canCallGuest&&!this.#poisoned&&this.#slot)this.#core._free(this.#slot);this.#slot=0;this.#poisoned=true;}
}
