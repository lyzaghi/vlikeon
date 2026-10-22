function actuator(pad){const value=pad?.vibrationActuator;return value&&typeof value.playEffect==='function'&&typeof value.reset==='function'&&(value.effects?.includes('dual-rumble')||value.type==='dual-rumble')?value:null;}
async function bounded(operation){let timer;try{return await Promise.race([operation,new Promise((_,reject)=>{timer=setTimeout(()=>reject(new Error('Haptic operation timed out.')),500);})]);}finally{clearTimeout(timer);}}
export class GamepadHaptics{
  #ports;#active;#enabled=false;#states=Array(4).fill(null);
  constructor(ports,active){this.#ports=ports;this.#active=active;ports.addEventListener('removed',event=>this.#discard(event.detail.port));window.addEventListener('blur',()=>this.stop());document.addEventListener('visibilitychange',()=>{if(document.hidden)this.stop();});window.addEventListener('pagehide',()=>this.stop());}
  enable(value){this.stop();this.#enabled=value===true;this.#states=Array(4).fill(null);}
  available(){return this.#ports.update().map((pad,port)=>Boolean(this.#enabled&&actuator(pad)&&!this.#states[port]?.failed));}
  #discard(port){const state=this.#states[port];if(state){clearTimeout(state.timer);this.#reset(state);}this.#states[port]=null;}
  stop(){for(const state of this.#states)if(state){clearTimeout(state.timer);state.timer=null;state.strong=state.weak=0;state.at=0;this.#reset(state);}}
  update(values,ageMs=0){
    if(!this.#enabled||!this.#active()||document.hidden||!document.hasFocus()){this.stop();return;}
    if(!Number.isFinite(ageMs)||ageMs<0||ageMs>250||!Array.isArray(values)||values.length!==4||values.some(item=>!item||!Number.isInteger(item.strong)||!Number.isInteger(item.weak)||item.strong<0||item.strong>65535||item.weak<0||item.weak>65535)){this.stop();return;}
    const pads=this.#ports.update();
    for(let port=0;port<4;port++){const output=actuator(pads[port]);if(!output){this.#discard(port);continue;}let state=this.#states[port];if(!state){state={output,strong:0,weak:0,at:0,busy:false,resetting:false,motor:false,failed:false};this.#states[port]=state;}if(state.failed)continue;
      state.strong=values[port].strong;state.weak=values[port].weak;state.at=performance.now()-ageMs;if(!state.strong&&!state.weak)this.#reset(state);else void this.#pulse(port,state);
    }
  }
  #reset(state){
    if(!state.motor||state.resetting)return;state.resetting=true;state.motor=false;
    void(async()=>{try{await bounded(Promise.resolve(state.output.reset()));}catch{state.failed=true;}finally{state.resetting=false;}})();
  }
  async #pulse(port,state){
    if(state.busy||state.resetting||state.failed||!this.#enabled||!this.#active()||this.#states[port]!==state)return;
    const now=performance.now();if(now-state.at>250||!state.strong&&!state.weak){this.#reset(state);return;}
    const delay=50-(now-(state.lastPulse??-1000));if(delay>0){if(!state.timer)state.timer=setTimeout(()=>{state.timer=null;void this.#pulse(port,state);},delay);return;}state.lastPulse=now;state.busy=true;state.motor=true;
    try{const result=await bounded(Promise.resolve(state.output.playEffect('dual-rumble',{duration:Math.max(1,Math.min(100,250-(now-state.at))),startDelay:0,strongMagnitude:state.strong/65535,weakMagnitude:state.weak/65535})));if(result!=='complete'&&result!=='preempted'){state.failed=true;this.#reset(state);}}
    catch{state.failed=true;this.#reset(state);}
    finally{state.busy=false;if(!state.failed&&this.#states[port]===state&&performance.now()-state.at<=250&&(state.strong||state.weak)&&this.#active())void this.#pulse(port,state);}
  }
}
