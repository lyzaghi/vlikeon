const LIMIT=256*1024*1024,HEADER=24,MAGIC=new TextEncoder().encode('OPDS0001');
function invalid(message='The emulator returned an invalid disc state.'){throw new Error(message);}
export function discEnvelope(raw,disc){
  if(!(raw instanceof ArrayBuffer)||!raw.byteLength||raw.byteLength>LIMIT-HEADER)invalid('The disc-aware snapshot exceeds its save limit.');
  const output=new Uint8Array(raw.byteLength+HEADER),view=new DataView(output.buffer);output.set(MAGIC);view.setUint32(8,disc.count,true);view.setUint32(12,disc.index,true);view.setUint8(16,disc.ejected?1:0);view.setUint32(20,raw.byteLength,true);output.set(new Uint8Array(raw),HEADER);return output.buffer;
}
export class DiscControl {
  #core;#enabled;#expected;#active=false;
  get enabled(){return this.#enabled;}
  constructor(core,{enabled=false,expectedCount=null}={}){
    this.#core=core;this.#enabled=enabled===true;this.#expected=expectedCount;
    if(expectedCount!==null&&(!Number.isInteger(expectedCount)||expectedCount<1||expectedCount>8))invalid('The game disc list is invalid.');
    if(this.#enabled&&['_om_web_disc_clear','_om_web_disc_register','_om_web_disc_count','_om_web_disc_index','_om_web_disc_ejected','_om_web_disc_set_ejected','_om_web_disc_set_index'].some(key=>typeof core[key]!=='function'))invalid('This installed emulator does not support safe disc changes.');
    if(this.#enabled)core._om_web_disc_clear();
  }
  environment(code,data){
    if(!this.#enabled)return 0;
    if(code===57){if(!Number.isInteger(data)||data<=0||data%4||data+4>this.#core.HEAPU8.length)return 0;this.#core.HEAPU32[data>>>2]=1;return 1;}
    if(code!==13&&code!==58)return 0;
    if(data===0)return this.#core._om_web_disc_register(0)?1:0;
    const size=code===13?28:40;if(!Number.isInteger(data)||data<=0||data%4||data+size>this.#core.HEAPU8.length)return 0;
    for(let index=0;index<7;index++)if(!this.#core.HEAPU32[(data+index*4)>>>2])return 0;
    return this.#core._om_web_disc_register(data)?1:0;
  }
  activate(){this.#active=true;return this.status();}
  status(){
    if(!this.#enabled)return{supported:false,count:0,index:0,ejected:false};
    if(!this.#active)invalid('The game disc interface is not active.');
    const count=this.#core._om_web_disc_count()>>>0,index=this.#core._om_web_disc_index()>>>0,ejected=this.#core._om_web_disc_ejected();
    if(count<1||count>8||index>=count||![0,1].includes(ejected)||this.#expected!==null&&count!==this.#expected)invalid();return{supported:true,count,index,ejected:ejected===1};
  }
  same(expected){const value=this.status();return value.supported===expected.supported&&value.count===expected.count&&value.index===expected.index&&value.ejected===expected.ejected;}
  apply(target){
    const before=this.status();if(!before.supported||!target||target.count!==before.count||!Number.isInteger(target.index)||target.index<0||target.index>=before.count||typeof target.ejected!=='boolean')invalid('The requested disc does not belong to this game.');
    if(before.index===target.index&&before.ejected===target.ejected)return before;
    if(!before.ejected&&!this.#core._om_web_disc_set_ejected(1))invalid('The emulator could not open its disc tray.');
    let value=this.status();if(!value.ejected||value.count!==before.count)invalid();
    if(value.index!==target.index&&!this.#core._om_web_disc_set_index(target.index))invalid('The emulator rejected the selected disc.');
    value=this.status();if(!value.ejected||value.index!==target.index)invalid();
    if(!target.ejected&&!this.#core._om_web_disc_set_ejected(0))invalid('The emulator could not close its disc tray.');
    if(!this.same(target))invalid();return this.status();
  }
  select(index){
    const before=this.status();if(!before.supported||!Number.isInteger(index)||index<0||index>=before.count)invalid('The requested disc does not belong to this game.');
    try{return this.apply({...before,index,ejected:false});}catch(error){try{this.apply(before);}catch{throw Object.assign(new Error('The emulator could not restore its disc tray after rejecting a disc change.'),{unrecoverable:true});}throw error;}
  }
  parse(payload){
    const bytes=payload instanceof ArrayBuffer?new Uint8Array(payload):payload;if(!(bytes instanceof Uint8Array)||bytes.byteLength<=HEADER||bytes.byteLength>LIMIT)invalid('The disc snapshot envelope is invalid.');
    const view=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);if(MAGIC.some((value,index)=>bytes[index]!==value)||bytes[16]>1||bytes[17]||bytes[18]||bytes[19]||view.getUint32(20,true)!==bytes.byteLength-HEADER)invalid('The disc snapshot envelope is invalid.');
    const current=this.status(),disc={supported:true,count:view.getUint32(8,true),index:view.getUint32(12,true),ejected:bytes[16]===1};
    if(!current.supported||disc.count!==current.count||disc.index>=disc.count)invalid('The snapshot disc list does not match this game.');return{raw:bytes.subarray(HEADER),disc};
  }
  clear(){this.#active=false;if(this.#enabled)this.#core._om_web_disc_clear();}
}
