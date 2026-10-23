import{currentLocale}from'./i18n.js';
import{BUTTON_NAMES}from'./input-profile.js';
import{defaultCustomTouchLayout}from'./touch-profile.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,label)=>{const value=document.createElement(tag);if(label!==undefined)value.textContent=label;return value;};
export function mountTouchLayoutEditor(root,getProfile,busy,changed){
  const abort=new AbortController(),listen=(target,type,callback)=>target.addEventListener(type,callback,{signal:abort.signal});
  const title=node('h4',text('Bố trí từng nút và cần','Place individual buttons and sticks'));
  const hint=node('p',text('Kéo từng nút trong khung. Có thể chọn nút rồi chỉnh X/Y, kích thước hoặc ẩn; phím mũi tên di chuyển nút đang chọn. Bố trí giữ tỉ lệ khi xoay/đổi cỡ màn hình. Lưu để áp dụng ở lần chơi tới.','Drag controls in the frame. Select a control to edit X/Y, size or visibility; arrow keys move the focused control. Positions scale when the screen changes. Save to apply next session.'));hint.className='muted small';
  const enabled=node('input');enabled.type='checkbox';const enabledLabel=node('label',text('Dùng bố trí tùy chỉnh','Use custom layout'));enabledLabel.prepend(enabled);
  const select=node('select');select.setAttribute('aria-label',text('Nút cần chỉnh','Control to edit'));
  const labels=[...BUTTON_NAMES,text('Cần trái','Left stick'),text('Cần phải','Right stick')];
  labels.forEach((label,id)=>{const option=node('option',label);option.value=String(id);select.append(option);});
  const preview=node('div');preview.className='touch-layout-editor';preview.setAttribute('role','group');preview.setAttribute('aria-label',text('Xem trước và kéo thả bố trí','Layout preview and drag editor'));
  const controls=labels.map((label,id)=>{const button=node('button',label);button.type='button';button.dataset.control=String(id);button.setAttribute('aria-label',label+' · '+text('kéo hoặc dùng phím mũi tên','drag or use arrow keys'));preview.append(button);return button;});
  const fields=[],form=node('div');form.className='touch-editor-fields';let selected=0,drag=null,resize=null;
  const current=()=>getProfile().custom_layout?.controls.find(v=>v.id===selected);
  for(const[key,label,min,max]of[['x','X',0,1000],['y','Y',0,1000],['size_percent',text('Kích thước %','Size %'),50,200],['height',text('Chiều cao khung /1000','Frame height /1000'),400,1200]]){
    const input=node('input'),out=node('output'),wrapper=node('label',label);input.type='range';input.min=String(min);input.max=String(max);input.step='1';input.setAttribute('aria-label',label);
    listen(input,'input',()=>{if(busy()||!current())return;if(key==='height')getProfile().custom_layout.height=Number(input.value);else current()[key]=Number(input.value);changed();});wrapper.append(input,out);form.append(wrapper);fields.push({key,input,out});
  }
  const visible=node('input');visible.type='checkbox';const visibleLabel=node('label',text('Hiển thị nút/cần này','Show this control'));visibleLabel.prepend(visible);form.append(visibleLabel);
  listen(visible,'change',()=>{if(!busy()&&current()){current().visible=visible.checked;changed();}});
  const reset=node('button',text('Đặt lại vị trí tùy chỉnh','Reset custom positions'));reset.type='button';reset.className='button button-secondary';
  listen(enabled,'change',()=>{if(busy())return;release();getProfile().custom_layout=enabled.checked?defaultCustomTouchLayout():null;changed();});
  listen(select,'change',()=>{release();selected=Number(select.value);paint();});
  listen(reset,'click',()=>{if(busy())return;release();getProfile().custom_layout=defaultCustomTouchLayout();changed();});
  function release(){const old=drag;drag=null;if(old)try{if(old.button.hasPointerCapture(old.id))old.button.releasePointerCapture(old.id);}catch{}}
  function move(event){
    if(!drag||event.pointerId!==drag.id)return;
    if(!event.isTrusted||busy()||!root.isConnected||document.hidden||!document.hasFocus()||!(event.buttons&1)){release();return;}
    const box=preview.getBoundingClientRect();if(box.width<=0||box.height<=0)return;
    const value=current();if(!value){release();return;}
    event.preventDefault();let x=Math.round((event.clientX-box.left)/box.width*1000),y=Math.round((event.clientY-box.top)/box.height*1000);
    if(getProfile().left_handed)x=1000-x;
    value.x=Math.max(0,Math.min(1000,x));value.y=Math.max(0,Math.min(1000,y));changed();
  }
  for(const[id,button]of controls.entries()){
    listen(button,'pointerdown',event=>{
      if(!event.isTrusted||busy()||drag||!getProfile().custom_layout||event.button!==0||!Number.isInteger(event.pointerId)||event.pointerId<0)return;
      event.preventDefault();selected=id;select.value=String(id);button.focus({preventScroll:true});
      try{button.setPointerCapture(event.pointerId);}catch{return;}drag={id:event.pointerId,button};paint();
    });
    listen(button,'pointermove',move);for(const event of['pointerup','pointercancel','lostpointercapture'])listen(button,event,value=>{if(drag?.id===value.pointerId)release();});
    listen(button,'click',()=>{selected=id;paint();});
    listen(button,'keydown',event=>{
      if(!event.isTrusted||busy()||!getProfile().custom_layout||event.ctrlKey||event.altKey||event.metaKey||!['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.code))return;
      event.preventDefault();event.stopPropagation();selected=id;const value=current(),step=event.shiftKey?10:1;
      const key=event.code==='ArrowLeft'||event.code==='ArrowRight'?'x':'y';let sign=event.code==='ArrowLeft'||event.code==='ArrowUp'?-1:1;
      if(key==='x'&&getProfile().left_handed)sign=-sign;value[key]=Math.max(0,Math.min(1000,value[key]+step*sign));changed();
    });
  }
  listen(window,'blur',release);listen(window,'pagehide',release);listen(document,'visibilitychange',()=>{if(document.hidden)release();});
  document.addEventListener('close',event=>{if(event.target instanceof Element&&event.target.contains(root))release();},{capture:true,signal:abort.signal});
  function paint(){
    const profile=getProfile(),layout=profile.custom_layout,value=current();enabled.checked=Boolean(layout);enabled.disabled=busy();select.value=String(selected);select.disabled=busy()||!layout;reset.disabled=busy()||!layout;preview.hidden=!layout;form.hidden=!layout;
    if(!layout){release();return;}
    if(busy())release();preview.style.aspectRatio='1000 / '+layout.height;preview.style.opacity=String(profile.opacity_percent/100);
    for(const field of fields){field.input.disabled=busy();field.input.value=String(field.key==='height'?layout.height:value[field.key]);field.out.textContent=field.input.value;}
    visible.checked=value.visible;visible.disabled=busy();const width=preview.clientWidth||320;
    layout.controls.forEach(placement=>{const button=controls[placement.id],base=placement.id>=16?110:44;
      const size=Math.min(width,width*layout.height/1000,Math.max(44,base*placement.size_percent/100*profile.size_percent/100));
      button.disabled=busy();button.dataset.selected=String(placement.id===selected);button.dataset.visible=String(placement.visible);
      const x=profile.left_handed?1000-placement.x:placement.x;
      Object.assign(button.style,{left:Math.max(size/2,Math.min(width-size/2,width*x/1000))+'px',top:Math.max(size/2,Math.min(width*layout.height/1000-size/2,width*layout.height/1000*placement.y/1000))+'px',width:size+'px',height:size+'px'});
    });
  }
  if(typeof ResizeObserver==='function'){resize=new ResizeObserver(()=>{release();paint();});resize.observe(preview);}
  root.append(title,hint,enabledLabel,select,preview,form,reset);
  // Disposal follows the settings panel, including refresh/replacement, rather
  // than leaving a document-level pointer capture/listener behind.
  const observer=new MutationObserver(()=>{if(!root.isConnected){abort.abort();resize?.disconnect();observer.disconnect();release();}});
  observer.observe(document.documentElement,{childList:true,subtree:true});paint();
  return{paint,dispose(){abort.abort();resize?.disconnect();observer.disconnect();release();}};
}
