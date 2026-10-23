import{currentLocale}from'./i18n.js';
import{BUTTON_NAMES}from'./input-profile.js';
import{defaultRawGamepad,calibratedAxis}from'./controller-profile.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,label)=>{const value=document.createElement(tag);if(label!==undefined)value.textContent=label;return value;};
const names=()=>[text('Trái X','Left X'),text('Trái Y','Left Y'),text('Phải X','Right X'),text('Phải Y','Right Y')];
const identity=v=>JSON.stringify([v.device_id??v.id,v.axis_count??v.axes.length,v.button_count??v.buttons.length]);
function pads(){try{return Array.from(navigator.getGamepads?.()??[]).slice(0,64).filter(v=>v?.connected&&typeof v.id==='string'&&v.id&&!/[\p{Cc}]/u.test(v.id)&&new TextEncoder().encode(v.id).length<=512&&v.axes.length<=32&&v.buttons.length<=64&&(v.axes.length||v.buttons.length));}catch{return[];}}
export function mountControllerSettings(root,getProfile,busy,changed){
  const abort=new AbortController(),axisFields=[];let formAbort=new AbortController();
  const listen=(target,event,handler)=>target.addEventListener(event,handler,{signal:abort.signal});
  const formListen=(target,event,handler)=>target.addEventListener(event,handler,{signal:formAbort.signal});
  const heading=node('h4',text('Tùy biến trục analog','Analog axis tuning')),axes=node('fieldset');axes.className='input-mapping-grid';
  names().forEach((name,index)=>{
    const row=node('div');row.className='controller-axis-row';row.append(node('strong',name));
    const source=node('select');names().forEach((label,i)=>{const option=node('option',label);option.value=String(i);source.append(option);});source.setAttribute('aria-label',name+' · '+text('Nguồn','Source'));
    listen(source,'change',()=>{if(busy())return;const list=getProfile().axis_tuning,old=list[index].source,other=list.findIndex(v=>v.source===Number(source.value));list[index].source=Number(source.value);if(other>=0&&other!==index)list[other].source=old;changed();});
    const invert=node('input');invert.type='checkbox';const invertLabel=node('label',text('Đảo chiều','Invert'));invertLabel.prepend(invert);listen(invert,'change',()=>{if(!busy()){getProfile().axis_tuning[index].inverted=invert.checked;changed();}});
    row.append(source,invertLabel);const fields={index,source,invert};
    for(const[key,label,min,max]of[['sensitivity_percent',text('Độ nhạy','Sensitivity'),50,200],['curve_percent',text('Đường cong','Response curve'),50,300]]){
      const range=node('input'),out=node('output'),wrapper=node('label',label);range.type='range';range.min=String(min);range.max=String(max);range.step='1';range.setAttribute('aria-label',name+' · '+label);
      listen(range,'input',()=>{if(!busy()){getProfile().axis_tuning[index][key]=Number(range.value);changed();}});wrapper.append(range,out);row.append(wrapper);fields[key]={range,out};
    }
    axisFields.push(fields);axes.append(row);
  });
  const title=node('h4',text('Hiệu chỉnh tay cầm qua Gamepad API','Gamepad API controller calibration'));
  const hint=node('p',text('Áp dụng cho phiên Web của Game Hub, kể cả tay cầm không có mapping chuẩn. SDL native dùng mapping thiết bị của SDL; các tùy biến trục phía trên áp dụng cho cả hai. Cấu hình chỉ lưu cục bộ cho game này.','Applies to Game Hub Web sessions, including nonstandard controllers. SDL native uses SDL device mappings; axis tuning above applies to both. Profiles stay local to this game.'));hint.className='muted small';
  const refresh=node('button',text('Tìm tay cầm','Refresh controllers')),select=node('select'),enable=node('button',text('Tạo cấu hình raw','Create raw profile')),remove=node('button',text('Xóa cấu hình raw','Remove raw profile'));
  for(const button of[refresh,enable,remove]){button.type='button';button.className='button button-secondary';}
  select.setAttribute('aria-label',text('Tay cầm cần hiệu chỉnh','Controller to calibrate'));
  const status=node('p'),readout=node('pre'),form=node('div');status.setAttribute('role','status');readout.className='controller-live-readout';readout.setAttribute('aria-label',text('Trục và nút hiện tại','Live axes and buttons'));
  const center=node('button',text('Ghi tâm khi thả cần','Capture released center')),range=node('button',text('Ghi biên trong 10 giây','Capture range for 10 seconds')),cancel=node('button',text('Dừng ghi','Stop capture'));
  for(const button of[center,range,cancel]){button.type='button';button.className='button button-secondary';}
  const captureActions=node('div');captureActions.className='detail-actions';captureActions.append(center,range,cancel);
  let selected='',lastProfile=null,frame=0,capture=null,fields=[],shown=false,disposed=false,connected=false;
  const current=()=>getProfile().raw_gamepads.find(v=>identity(v)===selected);
  const device=()=>pads().find(v=>identity(v)===selected);
  const stop=message=>{capture=null;cancel.disabled=true;if(message)status.textContent=message;};
  function refreshDevices(){
    const options=new Map();for(const pad of pads())options.set(identity(pad),pad.id+(pad.mapping==='standard'?' · standard':' · raw'));
    for(const profile of getProfile().raw_gamepads)if(!options.has(identity(profile)))options.set(identity(profile),profile.device_id+' · '+text('đã lưu, chưa kết nối','saved, disconnected'));
    select.replaceChildren();for(const[key,label]of options){const option=node('option',label);option.value=key;select.append(option);}
    if(!options.has(selected))selected=options.keys().next().value??'';select.value=selected;lastProfile=null;paint();
  }
  listen(refresh,'click',()=>{if(!busy()){stop();refreshDevices();}});
  listen(select,'change',()=>{stop();selected=select.value;lastProfile=null;paint();});
  listen(enable,'click',()=>{if(busy())return;const pad=device();if(!pad||current())return;const profile=getProfile();if(profile.raw_gamepads.length>=4){status.textContent=text('Tối đa 4 cấu hình. Xóa một cấu hình trước.','Four profiles maximum. Remove a profile first.');return;}profile.raw_gamepads.push(defaultRawGamepad(pad));lastProfile=null;changed();status.textContent=text('Cấu hình mới chưa gán nút. Chọn đúng nguồn nút trước khi lưu.','New profile has no button bindings. Select the correct sources before saving.');});
  listen(remove,'click',()=>{if(busy())return;stop();const list=getProfile().raw_gamepads,index=list.findIndex(v=>identity(v)===selected);if(index>=0)list.splice(index,1);lastProfile=null;changed();});
  function build(profile){
    formAbort.abort();formAbort=new AbortController();fields=[];form.replaceChildren();if(!profile)return;
    const axisHeading=node('h5',text('Trục raw: nguồn, min, tâm, max (−1000…1000)','Raw axes: source, min, center, max (−1000…1000)'));form.append(axisHeading);
    profile.axes.forEach((axis,index)=>{
      const row=node('div');row.className='controller-axis-row';row.append(node('strong',names()[index]));const source=node('select');
      for(let i=-1;i<profile.axis_count;i++){const option=node('option',i<0?text('Không dùng','Unused'):'Axis '+i);option.value=String(i);source.append(option);}source.value=String(axis.source??-1);
      formListen(source,'change',()=>{if(busy())return;stop();const old=axis.source,next=Number(source.value)<0?null:Number(source.value);const other=profile.axes.find(v=>v!==axis&&v.source===next&&next!==null);if(other)other.source=old;axis.source=next;lastProfile=null;changed();});row.append(source);fields.push(source);
      for(const key of['minimum','center','maximum']){const input=node('input');input.type='number';input.min='-1000';input.max='1000';input.step='1';input.value=String(axis[key]);input.setAttribute('aria-label',names()[index]+' '+key);formListen(input,'change',()=>{if(busy())return;stop();axis[key]=Number(input.value);changed();});row.append(input);fields.push(input);}
      form.append(row);
    });
    form.append(node('h5',text('Nguồn 16 nút game (nút raw hoặc chiều trục)','Sixteen game-button sources (raw button or axis direction)')));
    profile.buttons.forEach((binding,index)=>{
      const row=node('label',BUTTON_NAMES[index]);row.className='input-mapping-row';const source=node('select');
      const options=[['none',0,text('Chưa gán','Unassigned')]];
      for(let i=0;i<profile.button_count;i++)options.push(['button',i,'Button '+i]);
      for(let i=0;i<profile.axis_count;i++)for(const[kind,label]of[['axis_positive','+'],['axis_negative','−']])options.push([kind,i,'Axis '+i+' '+label]);
      for(const[kind,i,label]of options){const option=node('option',label);option.value=kind+':'+i;source.append(option);}source.value=binding.kind+':'+binding.index;
      formListen(source,'change',()=>{if(busy())return;const[kind,raw]=source.value.split(':');binding.kind=kind;binding.index=Number(raw);changed();});
      const threshold=node('input');threshold.type='number';threshold.min='1';threshold.max='100';threshold.value=String(binding.threshold_percent);threshold.setAttribute('aria-label',BUTTON_NAMES[index]+' '+text('ngưỡng %','threshold %'));formListen(threshold,'change',()=>{if(!busy()){binding.threshold_percent=Number(threshold.value);changed();}});
      row.append(source,threshold);form.append(row);fields.push(source,threshold);
    });
  }
  function paint(){
    if(capture&&(busy()||capture.profile!==current()))stop();
    for(const item of axisFields){const value=getProfile().axis_tuning[item.index];item.source.value=String(value.source);item.invert.checked=value.inverted;item.source.disabled=item.invert.disabled=busy();for(const key of['sensitivity_percent','curve_percent']){item[key].range.value=String(value[key]);item[key].out.textContent=value[key]+'%';item[key].range.disabled=busy();}}
    const profile=current();if(profile!==lastProfile){lastProfile=profile;build(profile);}for(const field of fields)field.disabled=busy();
    refresh.disabled=select.disabled=busy();enable.disabled=busy()||!device()||Boolean(profile);remove.disabled=busy()||!profile;center.disabled=range.disabled=busy()||!profile||!device();cancel.disabled=!capture;
  }
  listen(center,'click',()=>{
    if(busy())return;const profile=current(),pad=device();if(!profile||!pad)return;stop();
    for(const axis of profile.axes)if(axis.source!==null){const value=pad.axes[axis.source];if(Number.isFinite(value)&&value*1000-axis.minimum>=50&&axis.maximum-value*1000>=50)axis.center=Math.round(value*1000);}
    lastProfile=null;changed();status.textContent=text('Đã ghi tâm các trục hợp lệ vào bản nháp.','Captured valid released centers into the draft.');
  });
  listen(range,'click',()=>{
    if(busy())return;const pad=device();if(!pad||!current())return;
    capture={until:performance.now()+10000,minimum:pad.axes.map(v=>Number.isFinite(v)?Math.max(-1000,Math.min(1000,Math.round(v*1000))):0),maximum:pad.axes.map(v=>Number.isFinite(v)?Math.max(-1000,Math.min(1000,Math.round(v*1000))):0),key:selected,profile:current()};
    status.textContent=text('Di chuyển hai cần hết biên theo mọi hướng trong 10 giây. Không nhấn Lưu khi đang ghi.','Move both sticks through their full ranges for 10 seconds. Do not save during capture.');paint();
  });
  listen(cancel,'click',()=>{stop(text('Đã hủy ghi biên.','Range capture cancelled.'));paint();});
  listen(window,'blur',()=>{stop();});listen(document,'visibilitychange',()=>{if(document.hidden)stop();});
  function poll(){
    if(disposed||!root.isConnected){dispose();return;}
    const open=!root.closest('dialog')||root.closest('dialog').open;
    if(!open){stop();shown=false;}else if(!document.hidden&&document.hasFocus()){
      const pad=device(),profile=current();
      if(capture&&(busy()||!pad||capture.key!==selected||capture.profile!==profile))stop(text('Thiết bị hoặc cấu hình đã đổi; hủy ghi.','Device or settings changed; capture cancelled.'));
      if(capture){
        pad.axes.forEach((value,i)=>{if(Number.isFinite(value)){const n=Math.max(-1000,Math.min(1000,Math.round(value*1000)));capture.minimum[i]=Math.min(capture.minimum[i],n);capture.maximum[i]=Math.max(capture.maximum[i],n);}});
        if(performance.now()>=capture.until){let accepted=0;for(const axis of profile.axes)if(axis.source!==null&&axis.center-capture.minimum[axis.source]>=50&&capture.maximum[axis.source]-axis.center>=50){axis.minimum=capture.minimum[axis.source];axis.maximum=capture.maximum[axis.source];accepted++;}stop();lastProfile=null;changed();status.textContent=text('Đã ghi biên '+accepted+' trục hợp lệ. Nhấn Lưu để giữ lại.','Captured '+accepted+' valid axis ranges. Save to keep them.');}
      }
      readout.textContent=pad?pad.axes.map((v,i)=>'Axis '+i+': '+(Number.isFinite(v)?v.toFixed(3):'invalid')).join('  ')+'\n'+text('Nút đang nhấn: ','Pressed buttons: ')+pad.buttons.map((v,i)=>v.pressed?i:null).filter(v=>v!==null).join(', ')+(profile?'\n'+text('Sau hiệu chỉnh: ','Calibrated: ')+profile.axes.map(v=>calibratedAxis(v.source===null?0:pad.axes[v.source],v).toFixed(3)).join('  '):''):text('Chưa kết nối. Bấm một nút trên tay cầm rồi nhấn Tìm tay cầm.','Disconnected. Press a controller button, then refresh.');
      if(!shown||connected!==Boolean(pad)){shown=true;connected=Boolean(pad);paint();}
    }else stop();
    frame=requestAnimationFrame(poll);
  }
  root.append(heading,axes,title,hint,refresh,select,enable,remove,readout,captureActions,form,status);refreshDevices();frame=requestAnimationFrame(poll);
  function dispose(){disposed=true;abort.abort();formAbort.abort();cancelAnimationFrame(frame);stop();}
  return{paint,dispose};
}
