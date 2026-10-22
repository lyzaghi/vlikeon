// Keep a remaining controller in its existing guest port when another disconnects.
export class GamepadPorts extends EventTarget{
  #slots=Array(4).fill(null);
  constructor(){super();window.addEventListener('gamepaddisconnected',event=>{if(event.isTrusted)this.#forget(event.gamepad.index);});}
  #forget(index){for(let port=0;port<4;port++)if(this.#slots[port]?.index===index){this.#slots[port]=null;this.dispatchEvent(new CustomEvent('removed',{detail:{port}}));}}
  clear(){for(const item of this.#slots)if(item)this.#forget(item.index);}
  update(){
    let pads;try{pads=Array.from(navigator.getGamepads?.()??[]).slice(0,64);}catch{pads=[];}
    const available=pads.filter(pad=>pad?.connected&&pad.mapping==='standard'&&Number.isInteger(pad.index)&&pad.index>=0&&typeof pad.id==='string'&&pad.id.length<=512);
    for(let port=0;port<4;port++){const old=this.#slots[port];if(old&&!available.some(pad=>pad.index===old.index&&pad.id===old.id))this.#forget(old.index);}
    for(const pad of available){if(this.#slots.some(item=>item?.index===pad.index))continue;const slot=this.#slots.indexOf(null);if(slot<0)break;this.#slots[slot]={index:pad.index,id:pad.id};}
    return this.#slots.map(item=>item?available.find(pad=>pad.index===item.index&&pad.id===item.id)??null:null);
  }
}
