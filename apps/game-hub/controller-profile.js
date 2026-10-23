// Local, explicit device calibration. No raw device is guessed into a standard
// layout and no sample history or hardware identifier is sent to telemetry.
export const defaultAxisTuning=()=>Array.from({length:4},(_,source)=>({source,inverted:false,sensitivity_percent:100,curve_percent:100}));
const integer=(v,min,max)=>Number.isInteger(v)&&v>=min&&v<=max;
const object=(v,keys)=>v&&typeof v==='object'&&!Array.isArray(v)&&Object.keys(v).length===keys.length&&keys.every(k=>Object.hasOwn(v,k));
export function validateAxisTuning(value){
  if(!Array.isArray(value)||value.length!==4||value.some(v=>!object(v,['source','inverted','sensitivity_percent','curve_percent'])||!integer(v.source,0,3)||typeof v.inverted!=='boolean'||!integer(v.sensitivity_percent,50,200)||!integer(v.curve_percent,50,300))||new Set(value.map(v=>v.source)).size!==4)throw new Error('Invalid axis tuning.');
  return value.map(v=>({...v}));
}
export function validateRawGamepads(value){
  if(!Array.isArray(value)||value.length>4)throw new Error('At most four raw controller profiles are supported.');
  const keys=new Set();
  return value.map(device=>{
    if(!object(device,['device_id','axis_count','button_count','axes','buttons'])||typeof device.device_id!=='string'||!device.device_id||new TextEncoder().encode(device.device_id).length>512||/[\p{Cc}]/u.test(device.device_id)||!integer(device.axis_count,0,32)||!integer(device.button_count,0,64)||!device.axis_count&&!device.button_count||!Array.isArray(device.axes)||device.axes.length!==4||!Array.isArray(device.buttons)||device.buttons.length!==16)throw new Error('Invalid raw controller identity.');
    const key=JSON.stringify([device.device_id,device.axis_count,device.button_count]);if(keys.has(key))throw new Error('Duplicate raw controller profile.');keys.add(key);
    const sources=new Set();
    const axes=device.axes.map(v=>{
      if(!object(v,['source','minimum','center','maximum'])||v.source!==null&&(!integer(v.source,0,device.axis_count-1)||sources.has(v.source))||!integer(v.minimum,-1000,1000)||!integer(v.center,-1000,1000)||!integer(v.maximum,-1000,1000)||v.center-v.minimum<50||v.maximum-v.center<50)throw new Error('Invalid raw axis calibration.');
      if(v.source!==null)sources.add(v.source);return{...v};
    });
    const buttons=device.buttons.map(v=>{
      if(!object(v,['kind','index','threshold_percent'])||!integer(v.threshold_percent,1,100)||!['none','button','axis_positive','axis_negative'].includes(v.kind)||!integer(v.index,0,v.kind==='none'?0:v.kind==='button'?device.button_count-1:device.axis_count-1))throw new Error('Invalid raw button source.');return{...v};
    });
    return{device_id:device.device_id,axis_count:device.axis_count,button_count:device.button_count,axes,buttons};
  });
}
export function calibratedAxis(raw,calibration){
  if(calibration.source===null||!Number.isFinite(raw))return 0;
  const value=Math.max(-1000,Math.min(1000,raw*1000)),offset=value-calibration.center;
  return Math.max(-1,Math.min(1,offset/(offset<0?calibration.center-calibration.minimum:calibration.maximum-calibration.center)));
}
export const tuneAxis=(value,tuning)=>Math.max(-1,Math.min(1,Math.sign(value)*(tuning.inverted?-1:1)*Math.pow(Math.abs(value),tuning.curve_percent/100)*tuning.sensitivity_percent/100));
export function rawProfileFor(pad,profiles){return profiles.find(v=>v.device_id===pad.id&&v.axis_count===pad.axes.length&&v.button_count===pad.buttons.length);}
export function defaultRawGamepad(pad){
  return{device_id:pad.id,axis_count:pad.axes.length,button_count:pad.buttons.length,
    axes:Array.from({length:4},(_,i)=>({source:i<pad.axes.length?i:null,minimum:-1000,center:0,maximum:1000})),
    buttons:Array.from({length:16},()=>({kind:'none',index:0,threshold_percent:50}))};
}
export function normalizeGamepad(pad,profiles){
  const profile=rawProfileFor(pad,profiles);
  if(!profile)return pad.mapping==='standard'?pad:null;
  const positions=[0,2,8,9,12,13,14,15,1,3,4,5,6,7,10,11];
  const buttons=Array.from({length:17},()=>({pressed:false,touched:false,value:0}));
  profile.buttons.forEach((binding,index)=>{
    let value=0;
    if(binding.kind==='button')value=pad.buttons[binding.index]?.value??0;
    else if(binding.kind==='axis_positive')value=Math.max(0,pad.axes[binding.index]??0);
    else if(binding.kind==='axis_negative')value=Math.max(0,-(pad.axes[binding.index]??0));
    value=Number.isFinite(value)?Math.max(0,Math.min(1,value)):0;
    buttons[positions[index]]={pressed:value>=binding.threshold_percent/100,touched:value>0,value};
  });
  // Preserve the actual actuator object; mapping is never a permission to
  // fabricate an effect provider or to use an unrelated controller.
  return{index:pad.index,id:pad.id,connected:pad.connected,mapping:'standard',timestamp:pad.timestamp,
    axes:profile.axes.map(v=>calibratedAxis(v.source===null?0:pad.axes[v.source],v)),buttons,vibrationActuator:pad.vibrationActuator};
}
