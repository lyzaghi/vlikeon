import{currentLocale}from'./i18n.js';
import{defaultTouchProfile,validateTouchProfile}from'./touch-profile.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,className,value)=>{const item=document.createElement(tag);if(className)item.className=className;if(value!==undefined)item.textContent=value;return item;};
const clamp=value=>Math.max(-1,Math.min(1,value));
export class TouchControls{
  #root;#active;#system;#profile=defaultTouchProfile();#abort=new AbortController();#pointers=new Map();#keys=new Map();#pulses=new Map();#buttons=[];#sticks=[];#stickKeys=[new Set(),new Set()];#enabled=null;#showSticks=false;
  constructor(system,active){
    this.#system=system;this.#active=active;this.#root=node('div','touch-controller');this.#root.setAttribute('role','group');this.#root.setAttribute('aria-label',text('Điều khiển cảm ứng trong game','Game touch controls'));
    const main=node('div','touch-controller-main'),dpad=node('div','touch-direction-pad'),faces=node('div','touch-face-buttons'),aux=node('div','touch-aux-buttons'),sticks=node('div','touch-stick-pair');
    dpad.setAttribute('role','group');dpad.setAttribute('aria-label',text('Cụm hướng','Directional pad'));
    for(const[label,id]of[['↑',4],['↓',5],['←',6],['→',7]]){const button=this.#button(label,id);button.dataset.direction=String(id);dpad.append(button);}
    this.#listen(dpad,'pointerdown',event=>this.#begin(event,{kind:'dpad',target:dpad,buttons:[]}));
    this.#listen(dpad,'pointermove',event=>this.#move(event));
    for(const[label,id]of[['Y',1],['X',9],['B',0],['A',8]])faces.append(this.#button(label,id));
    for(const[label,id]of[['L',10],['L2',12],['Select',2],['Start',3],['R',11],['R2',13],['L3',14],['R3',15]])aux.append(this.#button(label,id));
    for(let index=0;index<2;index++){
      const pad=node('div','touch-analog-pad'),thumb=node('span','touch-analog-thumb'),label=node('span','touch-analog-label',index===0?text('Cần trái','Left stick'):text('Cần phải','Right stick'));thumb.setAttribute('aria-hidden','true');pad.tabIndex=0;pad.setAttribute('role','group');pad.setAttribute('aria-label',(index===0?text('Cần trái','Left stick'):text('Cần phải','Right stick'))+text(' · dùng phím mũi tên khi có tiêu điểm',' · use arrow keys when focused'));pad.append(thumb,label);
      this.#listen(pad,'pointerdown',event=>this.#begin(event,{kind:'stick',target:pad,index,x:0,y:0,buttons:[]}));this.#listen(pad,'pointermove',event=>this.#move(event));
      this.#listen(pad,'keydown',event=>{if(!event.isTrusted||!this.#acceptKeys(event)||!['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'].includes(event.code))return;event.preventDefault();event.stopPropagation();this.#stickKeys[index].add(event.code);this.#paint();});
      this.#listen(pad,'keyup',event=>{if(this.#stickKeys[index].delete(event.code)){event.preventDefault();event.stopPropagation();this.#paint();}});this.#listen(pad,'blur',()=>{this.#stickKeys[index].clear();this.#paint();});this.#sticks.push(pad);sticks.append(pad);
    }
    for(const type of['pointerup','pointercancel','lostpointercapture'])this.#listen(this.#root,type,event=>this.#release(event.pointerId));
    this.#listen(this.#root,'pointermove',event=>{if(this.#pointers.get(event.pointerId)?.kind==='button')this.#move(event);});
    main.append(dpad,faces);this.#root.append(main,aux,sticks);
    this.#listen(window,'blur',()=>this.clear());this.#listen(window,'pagehide',()=>this.clear());this.#listen(document,'visibilitychange',()=>{if(document.hidden)this.clear();});this.#listen(window,'resize',()=>{this.clear();this.#visibility();});
    const media=window.matchMedia('(any-pointer: coarse)');this.#listen(media,'change',()=>{this.clear();this.#visibility();});this.configure(this.#profile);
  }
  get element(){return this.#root;}
  #listen(target,type,handler){target.addEventListener(type,handler,{signal:this.#abort.signal});}
  configure(value){this.clear();this.#profile=validateTouchProfile(value);this.#root.classList.toggle('left-handed',this.#profile.left_handed);this.#root.style.setProperty('--touch-scale',String(this.#profile.size_percent/100));this.#root.style.setProperty('--touch-opacity',String(this.#profile.opacity_percent/100));this.#showSticks=this.#profile.layout==='sticks'||this.#profile.layout==='auto'&&['n64','psp'].includes(this.#system);this.#root.querySelector('.touch-stick-pair').hidden=!this.#showSticks;this.#visibility();}
  #visibility(){this.#root.hidden=this.#profile.visibility==='hidden'||this.#profile.visibility==='auto'&&!navigator.maxTouchPoints&&!window.matchMedia('(any-pointer: coarse)').matches;this.#sync();}
  #sync(){const enabled=Boolean(this.#active()&&!this.#root.hidden&&this.#root.isConnected&&!document.hidden&&document.hasFocus());if(this.#enabled!==enabled){this.#enabled=enabled;if(!enabled)this.clear();for(const{button}of this.#buttons)button.disabled=!enabled;for(const pad of this.#sticks)pad.tabIndex=enabled?0:-1;this.#root.setAttribute('aria-disabled',String(!enabled));}return enabled;}
  #acceptKeys(event){return this.#sync()&&!event.ctrlKey&&!event.altKey&&!event.metaKey;}
  #button(label,id){
    const button=node('button','touch-game-button',label);button.type='button';button.disabled=true;button.setAttribute('aria-label',text('Nút game ','Game button ')+label);button.setAttribute('aria-pressed','false');this.#buttons.push({button,id});let lastKeyboard=-1000;
    if(![4,5,6,7].includes(id))this.#listen(button,'pointerdown',event=>this.#begin(event,{kind:'button',target:button,buttons:[id]}));
    this.#listen(button,'keydown',event=>{if(!event.isTrusted||!this.#acceptKeys(event)||!['Space','Enter','NumpadEnter'].includes(event.code))return;event.preventDefault();event.stopPropagation();lastKeyboard=performance.now();if(!event.repeat&&this.#keys.size<32)this.#keys.set(id+':'+event.code,[id]);this.#paint();});
    this.#listen(button,'keyup',event=>{if(this.#keys.delete(id+':'+event.code)){event.preventDefault();event.stopPropagation();lastKeyboard=performance.now();this.#paint();}});
    this.#listen(button,'blur',()=>{for(const key of this.#keys.keys())if(key.startsWith(id+':'))this.#keys.delete(key);this.#paint();});
    this.#listen(button,'click',event=>{event.preventDefault();if(!event.isTrusted||event.detail!==0||performance.now()-lastKeyboard<300||!this.#sync())return;const old=this.#pulses.get(id);if(old)clearTimeout(old.timer);const timer=setTimeout(()=>{this.#pulses.delete(id);this.#paint();},120);this.#pulses.set(id,{buttons:[id],timer});this.#paint();});return button;
  }
  #begin(event,record){
    if(!event.isTrusted||!this.#sync()||event.button!==0||!Number.isInteger(event.pointerId)||event.pointerId<0||this.#pointers.size>=16||this.#pointers.has(event.pointerId))return;
    if(record.kind==='dpad'&&Array.from(this.#pointers.values()).some(item=>item.kind==='dpad')||record.kind==='stick'&&Array.from(this.#pointers.values()).some(item=>item.kind==='stick'&&item.index===record.index))return;
    event.preventDefault();try{record.target.setPointerCapture(event.pointerId);}catch{return;}this.#pointers.set(event.pointerId,record);this.#move(event);this.#paint();
  }
  #move(event){
    const record=this.#pointers.get(event.pointerId);if(!record||!event.isTrusted)return;if(!this.#sync()){this.clear();return;}if(!(event.buttons&1)){this.#release(event.pointerId);return;}if(record.kind==='button')return;
    const box=record.target.getBoundingClientRect();if(box.width<=0||box.height<=0){this.#release(event.pointerId);return;}event.preventDefault();const x=clamp((event.clientX-box.left-box.width/2)/(box.width*.36)),y=clamp((event.clientY-box.top-box.height/2)/(box.height*.36));
    if(record.kind==='stick'){record.x=x;record.y=y;}else{record.buttons=[];if(x<-.3)record.buttons.push(6);if(x>.3)record.buttons.push(7);if(y<-.3)record.buttons.push(4);if(y>.3)record.buttons.push(5);}this.#paint();
  }
  #release(id){if(this.#pointers.delete(id))this.#paint();}
  #held(){const buttons=new Set();for(const record of this.#pointers.values())for(const id of record.buttons)buttons.add(id);for(const values of this.#keys.values())for(const id of values)buttons.add(id);for(const record of this.#pulses.values())for(const id of record.buttons)buttons.add(id);return buttons;}
  #axes(index){if(!this.#showSticks)return{active:false,x:0,y:0};const pointer=Array.from(this.#pointers.values()).find(item=>item.kind==='stick'&&item.index===index);if(pointer)return{active:true,x:pointer.x,y:pointer.y};const keys=this.#stickKeys[index];return{active:keys.size>0,x:Number(keys.has('ArrowRight'))-Number(keys.has('ArrowLeft')),y:Number(keys.has('ArrowDown'))-Number(keys.has('ArrowUp'))};}
  #paint(){const buttons=this.#held();for(const{button,id}of this.#buttons){const held=buttons.has(id);button.dataset.held=String(held);button.setAttribute('aria-pressed',String(held));}for(let index=0;index<this.#sticks.length;index++){const value=this.#axes(index),pad=this.#sticks[index];pad.style.setProperty('--stick-x',value.x*35+'%');pad.style.setProperty('--stick-y',value.y*35+'%');}}
  snapshot(){if(!this.#sync())return{buttons:new Set(),sticks:[{active:false,x:0,y:0},{active:false,x:0,y:0}]};return{buttons:this.#held(),sticks:[this.#axes(0),this.#axes(1)]};}
  suspend(){this.clear();this.#enabled=false;for(const{button}of this.#buttons)button.disabled=true;for(const pad of this.#sticks)pad.tabIndex=-1;this.#root.setAttribute('aria-disabled','true');}
  clear(){const previous=Array.from(this.#pointers.entries());this.#pointers.clear();this.#keys.clear();this.#stickKeys.forEach(keys=>keys.clear());for(const record of this.#pulses.values())clearTimeout(record.timer);this.#pulses.clear();for(const[id,record]of previous)try{if(record.target.hasPointerCapture(id))record.target.releasePointerCapture(id);}catch{}this.#paint();}
  dispose(){this.#abort.abort();this.clear();this.#root.remove();}
}
