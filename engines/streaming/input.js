import{requireValue,exactKeys}from'./crypto.js';
import{REMOTE_KEYS}from'./control-profile.js';
const KEYS=new Set(REMOTE_KEYS);
const integer=(value,min,max)=>Number.isSafeInteger(value)&&value>=min&&value<=max;
export function validateInput(value){
  requireValue(exactKeys(value,['schema_version','keys','mouse','pads'])&&value.schema_version===1&&Array.isArray(value.keys)&&value.keys.length<=32&&new Set(value.keys).size===value.keys.length&&value.keys.every(key=>KEYS.has(key))&&exactKeys(value.mouse,['dx','dy','wheel','buttons'])&&['dx','dy','wheel'].every(key=>integer(value.mouse[key],-2048,2048))&&integer(value.mouse.buttons,0,7)&&Array.isArray(value.pads)&&value.pads.length<=4&&value.pads.every(pad=>exactKeys(pad,['axes','buttons'])&&Array.isArray(pad.axes)&&pad.axes.length===6&&pad.axes.every(axis=>Number.isFinite(axis)&&axis>=-1&&axis<=1)&&integer(pad.buttons,0,65535)),'invalid_request','Remote input state is invalid.');return value;
}
const emptyMouse=()=>({dx:0,dy:0,wheel:0,buttons:0});
const bounded=value=>Math.max(-2048,Math.min(2048,value));
// Standard Web Gamepad indices are stable player slots, never a compacted
// list of connected devices. This sampling function grants no capability.
export function sampleStandardGamepads(source){
  const slots=Array.from({length:4},()=>({axes:[0,0,0,0,0,0],buttons:0}));
  if(!source||!Number.isSafeInteger(source.length)||source.length<0)return slots;
  const axis=value=>Number.isFinite(value)?Math.max(-1,Math.min(1,value)):0;
  const trigger=button=>Number.isFinite(button?.value)?Math.max(0,Math.min(1,button.value)):0;
  for(let slot=0;slot<Math.min(4,source.length);slot++){
    const pad=source[slot];
    if(!pad||pad.connected!==true||pad.index!==slot||pad.mapping!=='standard'||
       !pad.axes||pad.axes.length<4||!pad.buttons||pad.buttons.length<16)continue;
    let buttons=0;
    for(let index=0;index<16;index++)if(pad.buttons[index]?.pressed===true)buttons|=1<<index;
    slots[slot]={axes:[...Array.from({length:4},(_,index)=>axis(pad.axes[index])),
      trigger(pad.buttons[6]),trigger(pad.buttons[7])],buttons};
  }
  return slots;
}
export class RemoteInput extends EventTarget{
  #element;#session;#abort=new AbortController();#timer=0;#keys=new Set();#mouse=emptyMouse();#enabled=false;#closed=false;
  #sources=new Map();#controlRoot=null;
  constructor(element,session){
    super();this.#element=element;this.#session=session;const options={signal:this.#abort.signal};element.tabIndex=0;
    element.addEventListener('keydown',event=>{
      if(!event.isTrusted||!this.#enabled||!KEYS.has(event.code)||event.altKey||event.metaKey||event.ctrlKey)return;
      // Keep an ordinary keyboard route back to the local Stop/settings UI.
      if(event.code==='Tab'&&event.shiftKey){this.clear();return;}
      event.preventDefault();this.#keys.add(event.code);if(this.#keys.size>32)this.clear();
    },options);
    element.addEventListener('keyup',event=>{if(event.isTrusted&&this.#keys.delete(event.code))event.preventDefault();},options);
    element.addEventListener('pointerdown',event=>{if(!event.isTrusted||!this.#enabled)return;element.focus();if(event.pointerType!=='touch'){this.#mouse.buttons=event.buttons&7;try{element.setPointerCapture(event.pointerId);}catch{}}},options);
    element.addEventListener('pointerup',event=>{if(event.isTrusted&&event.pointerType!=='touch')this.#mouse.buttons=event.buttons&7;},options);
    window.addEventListener('pointerup',event=>{if(event.isTrusted&&event.pointerType!=='touch')this.#mouse.buttons=event.buttons&7;},options);
    for(const name of['pointercancel','lostpointercapture'])element.addEventListener(name,()=>{this.#mouse.buttons=0;},options);
    element.addEventListener('pointermove',event=>{
      if(!event.isTrusted||!this.#enabled||document.pointerLockElement!==element)return;
      if(Number.isFinite(event.movementX))this.#mouse.dx=bounded(this.#mouse.dx+event.movementX);
      if(Number.isFinite(event.movementY))this.#mouse.dy=bounded(this.#mouse.dy+event.movementY);
    },options);
    element.addEventListener('wheel',event=>{
      if(!event.isTrusted||!this.#enabled||document.activeElement!==element||!Number.isFinite(event.deltaY))return;
      event.preventDefault();this.#mouse.wheel=bounded(this.#mouse.wheel-event.deltaY);
    },{...options,passive:false});
    element.addEventListener('contextmenu',event=>{if(this.#enabled)event.preventDefault();},options);
    element.addEventListener('blur',()=>{this.#keys.clear();this.#mouse=emptyMouse();queueMicrotask(()=>{if(!this.#focused())this.clear();});},options);
    window.addEventListener('blur',()=>this.clear(),options);window.addEventListener('pagehide',()=>this.clear(),options);
    // Release the old full state before any newly attached/replaced pad can
    // publish under the existing policy. Never compact other player slots.
    for(const name of['gamepadconnected','gamepaddisconnected'])window.addEventListener(name,()=>this.clear(),options);
    document.addEventListener('visibilitychange',()=>{if(document.hidden)this.clear();},options);
    document.addEventListener('focusout',()=>queueMicrotask(()=>{if(!this.#focused())this.clear();}),options);
    document.addEventListener('pointerlockchange',()=>{if(document.pointerLockElement!==element)this.clear();},options);
    this.#timer=setInterval(()=>this.#send(),16);
  }
  get enabled(){return this.#enabled&&!this.#closed;}
  #focused(){return!document.hidden&&document.hasFocus()&&(document.activeElement===this.#element||this.#controlRoot?.contains(document.activeElement));}
  enable(value){if(this.#closed)return;this.#enabled=value===true;this.clear();}
  clear(){
    this.#keys.clear();this.#mouse=emptyMouse();
    for(const value of this.#sources.values()){value.keys.clear();Object.assign(value.mouse,emptyMouse());}
    this.#session.releaseInput();this.dispatchEvent(new Event('cleared'));
  }
  createOverlay(element){
    requireValue(!this.#closed&&this.#sources.size<4&&element?.ownerDocument===this.#element.ownerDocument&&typeof element.contains==='function'&&
      (!this.#controlRoot||this.#controlRoot===element),'invalid_state','Remote input overlays require one local control region.');
    this.#controlRoot=element;const id=Symbol(),value={keys:new Set(),mouse:emptyMouse()};this.#sources.set(id,value);
    const reset=()=>{value.keys.clear();Object.assign(value.mouse,emptyMouse());};
    return{
      setHeld:(keys,buttons=0)=>{
        requireValue(Array.isArray(keys)&&keys.length<=16&&keys.every(key=>KEYS.has(key))&&integer(buttons,0,7),'invalid_request','Remote overlay state is invalid.');
        if(!this.enabled||!this.#sources.has(id)){reset();return;}
        value.keys=new Set(keys);value.mouse.buttons=buttons;
      },
      move:(dx,dy,wheel=0)=>{
        requireValue([dx,dy,wheel].every(n=>Number.isFinite(n)&&Math.abs(n)<=1000000),'invalid_request','Remote pointer motion is invalid.');
        if(!this.enabled||!this.#sources.has(id))return;
        value.mouse.dx=bounded(value.mouse.dx+dx);value.mouse.dy=bounded(value.mouse.dy+dy);value.mouse.wheel=bounded(value.mouse.wheel+wheel);
      },
      clear:reset,
      close:()=>{reset();this.#sources.delete(id);if(!this.#sources.size)this.#controlRoot=null;this.clear();},
    };
  }
  async lockPointer(){requireValue(this.enabled&&typeof this.#element.requestPointerLock==='function','unsupported_capability','Remote input or pointer lock is unavailable.');this.#element.focus();await this.#element.requestPointerLock();}
  #send(){
    if(!this.enabled||!this.#focused())return;
    const keys=new Set(this.#keys),mouse={dx:0,dy:0,wheel:0,buttons:this.#mouse.buttons};
    for(const value of[{keys:new Set(),mouse:this.#mouse},...this.#sources.values()]){
      for(const key of value.keys)keys.add(key);mouse.buttons|=value.mouse.buttons;
      for(const name of['dx','dy','wheel']){const whole=Math.trunc(value.mouse[name]);value.mouse[name]-=whole;mouse[name]=bounded(mouse[name]+whole);}
    }
    if(keys.size>32){this.clear();return;}
    let pads=[];
    if(this.#session.gamepadInputAvailable)try{pads=sampleStandardGamepads(navigator.getGamepads?.()??[]);}catch{this.clear();return;}
    try{this.#session.sendInput({schema_version:1,keys:[...keys],mouse,pads});}
    catch{this.#enabled=false;this.clear();this.dispatchEvent(new Event('disabled'));}
  }
  close(){if(this.#closed)return;this.clear();this.#closed=true;this.#enabled=false;clearInterval(this.#timer);this.#abort.abort();if(document.pointerLockElement===this.#element)void document.exitPointerLock();}
}
