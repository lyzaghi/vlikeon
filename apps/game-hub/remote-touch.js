import{currentLocale}from'./i18n.js';
import{defaultRemoteControls,validateRemoteControls,controlAction,REMOTE_KEYS,REMOTE_ACTIONS}from'./streaming/control-profile.js';

const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const arrows=['ArrowUp','ArrowDown','ArrowLeft','ArrowRight'];
function node(tag,classes,value){const element=document.createElement(tag);if(classes)element.className=classes;if(value!==undefined)element.textContent=value;return element;}
function label(title,input){const element=node('label','field-label');element.append(node('span','',title),input);return element;}
function actionName(value){return value==='mouse_primary'?text('Chuột trái','Left mouse'):value==='mouse_secondary'?text('Chuột phải','Right mouse'):value==='mouse_middle'?text('Chuột giữa','Middle mouse'):value.startsWith('Key')?value.slice(3):value.startsWith('Digit')?value.slice(5):value;}
function select(values,title){const element=node('select');element.setAttribute('aria-label',title);for(const value of values){const option=node('option','',actionName(value));option.value=value;element.append(option);}return element;}

// This controller maps local touch gestures to the host's existing keyboard/mouse
// permission. It never creates a virtual gamepad or stores an armed input state.
export class RemoteTouchControls{
  #input;#store;#pairId;#source;#abort=new AbortController();#closed=false;#loaded=false;#allowed=false;#active=false;#saving=false;
  #profile=defaultRemoteControls();#pointers=new Map();#moveKeys=new Set();#lookKeys=new Set();#actionKeys=new Set();#lookTimer=0;
  #toggle;#status;#form;#move;#look;#stick;#buttons=[];#fields={};
  element;panel;
  constructor(input,store,pairId){
    this.#input=input;this.#store=store;this.#pairId=pairId;const options={signal:this.#abort.signal};
    this.element=node('section','remote-touch');this.element.tabIndex=0;this.element.hidden=true;this.element.setAttribute('role','group');this.element.setAttribute('aria-label',text('Điều khiển cảm ứng game từ xa','Remote game touch controls'));
    this.#source=input.createOverlay(this.element);
    const header=node('div','remote-touch-header'),leave=node('button','button button-secondary',text('Ẩn cảm ứng','Hide touch controls'));leave.type='button';leave.addEventListener('click',()=>this.#activate(false),options);header.append(node('span','small',text('Cảm ứng → bàn phím/chuột','Touch → keyboard/mouse')),leave);
    const pads=node('div','remote-touch-pads');this.#move=node('div','remote-touch-move');this.#look=node('div','remote-touch-look');
    for(const[pad,title]of[[this.#move,text('Di chuyển: kéo hoặc dùng phím mũi tên','Move: drag or use arrow keys')],[this.#look,text('Nhìn: kéo hoặc dùng phím mũi tên','Look: drag or use arrow keys')]]){pad.tabIndex=0;pad.setAttribute('role','button');pad.setAttribute('aria-label',title);}
    this.#stick=node('span','remote-touch-stick');this.#move.append(node('span','remote-touch-pad-title',text('Di chuyển','Move')),this.#stick);this.#look.append(node('span','remote-touch-pad-title',text('Kéo để nhìn','Drag to look')));
    const actions=node('div','remote-touch-actions');for(let index=0;index<8;index++){const button=node('button','remote-touch-action');button.type='button';button.setAttribute('aria-pressed','false');this.#buttons.push(button);actions.append(button);this.#bindPointer(button,'action',index);this.#bindActionKey(button,index);}
    pads.append(this.#move,this.#look,actions);this.element.append(header,pads);this.#bindPointer(this.#move,'move');this.#bindPointer(this.#look,'look');this.#bindPadKeys(this.#move,this.#moveKeys);this.#bindPadKeys(this.#look,this.#lookKeys);
    this.element.addEventListener('contextmenu',event=>event.preventDefault(),options);
    for(const name of['pointerup','pointercancel'])window.addEventListener(name,event=>this.#releasePointer(event.pointerId),options);
    input.addEventListener('cleared',()=>this.#clearLocal(),options);input.addEventListener('disabled',()=>this.enable(false),options);window.addEventListener('resize',()=>input.clear(),options);
    this.panel=node('section','remote-touch-panel');this.#toggle=node('button','button button-secondary',text('Bật điều khiển cảm ứng','Enable touch controls'));this.#toggle.type='button';this.#toggle.disabled=true;this.#toggle.setAttribute('aria-pressed','false');this.#toggle.addEventListener('click',()=>this.#activate(!this.#active),options);
    this.#status=node('p','muted small');this.#status.setAttribute('role','status');const details=node('details'),summary=node('summary','',text('Tùy chỉnh cảm ứng','Customize touch controls'));this.#form=node('form','remote-touch-settings');
    const directions=[text('Lên','Up'),text('Xuống','Down'),text('Trái','Left'),text('Phải','Right')];this.#fields.move=directions.map(title=>{const element=select(REMOTE_KEYS,title);this.#form.append(label(title,element));return element;});
    this.#fields.actions=Array.from({length:8},(_,index)=>{const title=`${text('Nút','Button')} ${index+1}`,element=select(REMOTE_ACTIONS,title);this.#form.append(label(title,element));return element;});
    for(const[name,title,min,max]of[['look_sensitivity_percent',text('Độ nhạy nhìn (%)','Look sensitivity (%)'),25,300],['size_percent',text('Cỡ điều khiển (%)','Control size (%)'),80,140],['opacity_percent',text('Độ rõ (%)','Opacity (%)'),40,100]]){const element=node('input');element.type='number';element.min=String(min);element.max=String(max);element.step='1';element.required=true;this.#fields[name]=element;this.#form.append(label(title,element));}
    for(const[name,title]of[['invert_y',text('Đảo trục nhìn dọc','Invert vertical look')],['left_handed',text('Đổi bên cho tay trái','Swap sides for left-handed use')]]){const element=node('input');element.type='checkbox';this.#fields[name]=element;this.#form.append(label(title,element));}
    const save=node('button','button',text('Lưu tùy chỉnh','Save controls'));save.type='submit';const defaults=node('button','button button-secondary',text('Điền mặc định','Fill defaults'));defaults.type='button';defaults.addEventListener('click',()=>this.#fill(defaultRemoteControls()),options);this.#form.append(save,defaults);this.#form.addEventListener('submit',event=>{event.preventDefault();void this.#save();},options);details.append(summary,this.#form);this.panel.append(this.#toggle,this.#status,details);this.#fill(this.#profile);this.#apply();
    // Settings are local preferences. Opening the editor releases held input.
    details.addEventListener('toggle',()=>{if(details.open)this.#input.clear();},options);
  }
  get active(){return this.#active&&!this.#closed;}
  async load(){
    try{const profile=await this.#store.controlProfile(this.#pairId);if(this.#closed)return;this.#profile=profile;this.#fill(profile);this.#apply();this.#status.textContent=text('Cảm ứng dùng quyền bàn phím/chuột của host.','Touch uses the host’s keyboard/mouse authorization.');}
    catch{if(this.#closed)return;this.#status.textContent=text('Không đọc được tùy chỉnh. Bạn có thể lưu lại cấu hình mặc định.','Controls could not be loaded. You can save the default configuration again.');}
    if(this.#closed)return;this.#loaded=true;this.#toggle.disabled=!this.#allowed;
  }
  enable(value){if(this.#closed)return;this.#allowed=value===true;this.#toggle.disabled=!this.#loaded||!this.#allowed||this.#saving;if(!this.#allowed)this.#activate(false);}
  #activate(value){
    if(this.#closed)return;this.#input.clear();this.#active=value===true&&this.#loaded&&this.#allowed&&!this.#saving&&this.#input.enabled;this.element.hidden=!this.#active;this.#toggle.setAttribute('aria-pressed',String(this.#active));this.#toggle.textContent=this.#active?text('Ẩn điều khiển cảm ứng','Hide touch controls'):text('Bật điều khiển cảm ứng','Enable touch controls');if(this.#active)this.element.focus({preventScroll:true});
  }
  #usable(){return!this.#closed&&this.#active&&this.#allowed&&this.#input.enabled&&this.element.isConnected&&!document.hidden&&document.hasFocus();}
  #bindPointer(element,kind,index=0){
    const options={signal:this.#abort.signal};element.addEventListener('pointerdown',event=>{
      if(!event.isTrusted||!this.#usable()||event.button!==0||this.#pointers.size>=10||this.#pointers.has(event.pointerId)||kind!=='action'&&[...this.#pointers.values()].some(value=>value.kind===kind))return;
      event.preventDefault();element.focus({preventScroll:true});const value={element,kind,index,x:event.clientX,y:event.clientY,keys:[]};this.#pointers.set(event.pointerId,value);
      try{element.setPointerCapture(event.pointerId);}catch{this.#pointers.delete(event.pointerId);return;}
      if(kind==='move')this.#movePointer(value,event);this.#publish();
    },options);
    element.addEventListener('pointermove',event=>{const value=this.#pointers.get(event.pointerId);if(!event.isTrusted||!value||value.element!==element||!this.#usable())return;event.preventDefault();if(kind==='move')this.#movePointer(value,event);else if(kind==='look'){const scale=this.#profile.look_sensitivity_percent/100,dx=(event.clientX-value.x)*scale,dy=(event.clientY-value.y)*scale*(this.#profile.invert_y?-1:1);if(Number.isFinite(dx)&&Number.isFinite(dy)&&Math.abs(dx)<=1000000&&Math.abs(dy)<=1000000)this.#source.move(dx,dy);value.x=event.clientX;value.y=event.clientY;}this.#publish();},options);
    element.addEventListener('lostpointercapture',event=>this.#releasePointer(event.pointerId),options);
  }
  #movePointer(value,event){
    const rect=value.element.getBoundingClientRect();if(rect.width<=0||rect.height<=0)return;const x=Math.max(-1,Math.min(1,(event.clientX-rect.left-rect.width/2)/(rect.width/2))),y=Math.max(-1,Math.min(1,(event.clientY-rect.top-rect.height/2)/(rect.height/2)));if(!Number.isFinite(x)||!Number.isFinite(y))return;
    value.keys=[];if(y<-.3)value.keys.push(this.#profile.move_keys[0]);if(y>.3)value.keys.push(this.#profile.move_keys[1]);if(x<-.3)value.keys.push(this.#profile.move_keys[2]);if(x>.3)value.keys.push(this.#profile.move_keys[3]);this.#stick.style.transform=`translate(${x*35}%, ${y*35}%)`;
  }
  #releasePointer(id){const value=this.#pointers.get(id);if(!value)return;this.#pointers.delete(id);try{if(value.element.hasPointerCapture(id))value.element.releasePointerCapture(id);}catch{}if(value.kind==='move')this.#stick.style.transform='';this.#publish();}
  #bindActionKey(element,index){
    const options={signal:this.#abort.signal};element.addEventListener('keydown',event=>{if(!event.isTrusted||!this.#usable()||!['Space','Enter'].includes(event.code)||event.altKey||event.ctrlKey||event.metaKey)return;event.preventDefault();this.#actionKeys.add(`${index}:${event.code}`);this.#publish();},options);
    element.addEventListener('keyup',event=>{if(!event.isTrusted)return;if(this.#actionKeys.delete(`${index}:${event.code}`)){event.preventDefault();this.#publish();}},options);
    element.addEventListener('blur',()=>{this.#actionKeys.delete(`${index}:Space`);this.#actionKeys.delete(`${index}:Enter`);this.#publish();},options);
  }
  #bindPadKeys(element,keys){
    const options={signal:this.#abort.signal};element.addEventListener('keydown',event=>{if(!event.isTrusted||!this.#usable()||!arrows.includes(event.code)||event.altKey||event.ctrlKey||event.metaKey)return;event.preventDefault();keys.add(event.code);this.#publish();if(keys===this.#lookKeys&&!this.#lookTimer)this.#lookTimer=setInterval(()=>this.#keyboardLook(),16);},options);
    element.addEventListener('keyup',event=>{if(event.isTrusted&&keys.delete(event.code)){event.preventDefault();this.#publish();this.#stopLook();}},options);element.addEventListener('blur',()=>{keys.clear();this.#publish();this.#stopLook();},options);
  }
  #keyboardLook(){if(!this.#usable()||document.activeElement!==this.#look){this.#lookKeys.clear();this.#stopLook();return;}const step=4*this.#profile.look_sensitivity_percent/100,dx=(Number(this.#lookKeys.has('ArrowRight'))-Number(this.#lookKeys.has('ArrowLeft')))*step,dy=(Number(this.#lookKeys.has('ArrowDown'))-Number(this.#lookKeys.has('ArrowUp')))*step*(this.#profile.invert_y?-1:1);this.#source.move(dx,dy);}
  #stopLook(){if(!this.#lookKeys.size){clearInterval(this.#lookTimer);this.#lookTimer=0;}}
  #publish(){
    if(this.#closed)return;const keys=new Set(),actions=new Set();for(const arrow of this.#moveKeys){const index=arrows.indexOf(arrow);if(index!==-1)keys.add(this.#profile.move_keys[index]);}for(const value of this.#pointers.values()){if(value.kind==='move')for(const key of value.keys)keys.add(key);if(value.kind==='action')actions.add(value.index);}for(const token of this.#actionKeys)actions.add(Number(token.split(':')[0]));
    let buttons=0;for(const index of actions){const value=controlAction(this.#profile.actions[index]);for(const key of value.keys)keys.add(key);buttons|=value.buttons;}this.#source.setHeld([...keys],buttons);this.#buttons.forEach((button,index)=>button.setAttribute('aria-pressed',String(actions.has(index))));
  }
  #clearLocal(){
    const pointers=[...this.#pointers.entries()];this.#pointers.clear();this.#moveKeys.clear();this.#lookKeys.clear();this.#actionKeys.clear();this.#stopLook();this.#source.clear();this.#stick.style.transform='';for(const button of this.#buttons)button.setAttribute('aria-pressed','false');for(const[id,value]of pointers)try{if(value.element.hasPointerCapture(id))value.element.releasePointerCapture(id);}catch{}
  }
  #fill(profile){this.#fields.move.forEach((element,index)=>{element.value=profile.move_keys[index];});this.#fields.actions.forEach((element,index)=>{element.value=profile.actions[index];});for(const name of['look_sensitivity_percent','size_percent','opacity_percent'])this.#fields[name].value=String(profile[name]);for(const name of['invert_y','left_handed'])this.#fields[name].checked=profile[name];}
  #apply(){this.element.classList.toggle('remote-touch-left',this.#profile.left_handed);this.element.style.setProperty('--remote-control-size',String(this.#profile.size_percent/100));this.element.style.setProperty('--remote-control-opacity',String(this.#profile.opacity_percent/100));this.#buttons.forEach((button,index)=>{button.textContent=actionName(this.#profile.actions[index]);button.setAttribute('aria-label',`${text('Nút','Button')} ${index+1}: ${actionName(this.#profile.actions[index])}`);});}
  async #save(){
    if(this.#closed||this.#saving||!this.#loaded)return;this.#activate(false);this.#saving=true;this.#toggle.disabled=true;for(const element of this.#form.elements)element.disabled=true;
    try{const value=validateRemoteControls({schema_version:1,move_keys:this.#fields.move.map(element=>element.value),actions:this.#fields.actions.map(element=>element.value),look_sensitivity_percent:Number(this.#fields.look_sensitivity_percent.value),size_percent:Number(this.#fields.size_percent.value),opacity_percent:Number(this.#fields.opacity_percent.value),invert_y:this.#fields.invert_y.checked,left_handed:this.#fields.left_handed.checked});const saved=await this.#store.setControlProfile(this.#pairId,value);if(this.#closed)return;this.#profile=saved;this.#fill(saved);this.#apply();this.#status.textContent=text('Đã lưu. Bật cảm ứng khi bạn muốn tiếp tục.','Saved. Enable touch controls when you want to continue.');}
    catch(error){if(!this.#closed)this.#status.textContent=error.code==='invalid_request'?text('Chọn bốn phím di chuyển khác nhau và giá trị trong giới hạn.','Choose four distinct movement keys and values within the supported limits.'):text('Không lưu được tùy chỉnh. Kiểm tra ghép nối rồi thử lại.','Controls could not be saved. Check pairing and try again.');}
    finally{if(!this.#closed){this.#saving=false;for(const element of this.#form.elements)element.disabled=false;this.#toggle.disabled=!this.#allowed;}}
  }
  close(){if(this.#closed)return;this.#activate(false);this.#closed=true;this.#clearLocal();this.#abort.abort();this.#source.close();this.element.remove();this.panel.remove();}
}
