import{normalizeGamepad,validateRawGamepads}from'./controller-profile.js';
import{PHYSICAL_BUTTONS}from'./input-profile.js';
// Keep a remaining controller in its existing guest port when another disconnects.
export class GamepadPorts extends EventTarget{
  #slots=Array(4).fill(null);
  #nativePads=Array(4).fill(null);
  #hapticsBridge=null;
  #profiles=[];
  configure(profiles){this.clear();this.#profiles=validateRawGamepads(profiles);}
  constructor(){super();window.addEventListener('gamepaddisconnected',event=>{if(event.isTrusted)this.#forget(event.gamepad.index);});}
  #forget(index){for(let port=0;port<4;port++)if(this.#slots[port]?.index===index){this.#slots[port]=null;if(!this.#nativePads[port])this.dispatchEvent(new CustomEvent('removed',{detail:{port}}));}}
  clearNativeSnapshot(){
    this.#hapticsBridge=null;
    for(let port=0;port<4;port++){
      if(this.#nativePads[port]){this.#nativePads[port]=null;this.dispatchEvent(new CustomEvent('removed',{detail:{port}}));}
    }
  }
  setNativeSnapshot(snapshot,hapticsBridge){
    this.#hapticsBridge=typeof hapticsBridge==='function'?hapticsBridge:null;
    const next=Array(4).fill(null);
    if(snapshot?.active===true&&Array.isArray(snapshot.ports)){
      for(const entry of snapshot.ports){
        if(!entry||entry.connected!==true||!Number.isInteger(entry.port)||entry.port<0||entry.port>=4)continue;
        const buttons=Array.from({length:17},()=>({pressed:false,touched:false,value:0}));
        const mask=Number.isInteger(entry.buttons_mask)?(entry.buttons_mask&0xffff):0;
        const pressure=Array.isArray(entry.pressure_permille)?entry.pressure_permille:[];
        for(let retro=0;retro<16;retro++){
          const phys=PHYSICAL_BUTTONS[retro];
          const pressed=(mask&(1<<retro))!==0;
          const rawPermille=Number.isFinite(pressure[retro])?Math.max(0,Math.min(1000,pressure[retro])):(pressed?1000:0);
          const value=pressed?Math.max(rawPermille/1000,0.001):(rawPermille/1000);
          buttons[phys]={pressed,touched:pressed||rawPermille>0,value:Math.min(1,value)};
        }
        const axes=[0,1,2,3].map(i=>Number.isFinite(entry.axes?.[i])?Math.max(-1,Math.min(1,entry.axes[i]/32767)):0);
        const portIndex=entry.port,sendHaptics=this.#hapticsBridge;
        const vibrationActuator=(entry.rumble_supported===true&&sendHaptics)?{
          type:'dual-rumble',
          effects:['dual-rumble'],
          playEffect:(_type,opts={})=>{
            const duration=Math.max(0,Math.min(250,Math.round(opts.duration||0)));
            const strong=Math.max(0,Math.min(65535,Math.round((opts.strongMagnitude||0)*65535)));
            const weak=Math.max(0,Math.min(65535,Math.round((opts.weakMagnitude||0)*65535)));
            return sendHaptics(portIndex,duration>0&&(strong>0||weak>0),weak,strong,duration);
          },
          reset:()=>sendHaptics(portIndex,false,0,0,0),
        }:null;
        next[portIndex]={index:1000+portIndex,id:typeof entry.label==='string'?entry.label.slice(0,128):`Mobile Controller ${portIndex+1}`,connected:true,mapping:'standard',buttons,axes,vibrationActuator};
      }
    }
    for(let port=0;port<4;port++){
      const had=Boolean(this.#nativePads[port]);
      this.#nativePads[port]=next[port];
      if(had&&!next[port])this.dispatchEvent(new CustomEvent('removed',{detail:{port}}));
    }
  }
  clear(){this.clearNativeSnapshot();for(const item of this.#slots)if(item)this.#forget(item.index);}
  update(){
    let pads;try{pads=Array.from(navigator.getGamepads?.()??[]).slice(0,64);}catch{pads=[];}
    const available=pads.filter(pad=>pad?.connected&&Number.isInteger(pad.index)&&pad.index>=0&&typeof pad.id==='string'&&pad.id.length<=512&&pad.axes?.length<=32&&pad.buttons?.length<=64).map(pad=>normalizeGamepad(pad,this.#profiles)).filter(Boolean);
    for(let port=0;port<4;port++){const old=this.#slots[port];if(old&&!available.some(pad=>pad.index===old.index&&pad.id===old.id))this.#forget(old.index);}
    for(const pad of available){if(this.#slots.some(item=>item?.index===pad.index))continue;const slot=this.#slots.indexOf(null);if(slot<0)break;this.#slots[slot]={index:pad.index,id:pad.id};}
    const webPorts=this.#slots.map(item=>item?available.find(pad=>pad.index===item.index&&pad.id===item.id)??null:null);
    return[0,1,2,3].map(port=>this.#nativePads[port]??webPorts[port]);
  }
}
