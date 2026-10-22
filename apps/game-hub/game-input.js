import{PHYSICAL_BUTTONS}from'./input-profile.js';
const axis=value=>Number.isFinite(value)?Math.max(-1,Math.min(1,value)):0;
const signed=value=>{const scaled=Math.max(-1,Math.min(1,value))*32767;return scaled<0?-Math.floor(-scaled+.5):Math.floor(scaled+.5);};
export function radialStick(rawX,rawY,deadzonePercent){
  if(!Number.isInteger(deadzonePercent)||deadzonePercent<0||deadzonePercent>50)return[0,0];
  const x=axis(rawX),y=axis(rawY),length=Math.hypot(x,y),deadzone=deadzonePercent/100;
  if(!length||length<=deadzone)return[0,0];const scale=(Math.min(1,length)-deadzone)/((1-deadzone)*length);return[signed(x*scale),signed(y*scale)];
}
export function gameInput(pads,profile,heldButtons,touchButtons,analogKeys,analogMode,virtualSticks=[]){
  const ports=Array.from({length:4},()=>Array(16).fill(0)),analogPorts=Array.from({length:4},()=>Array(4).fill(0));
  for(const id of heldButtons)ports[0][id]=1;for(const id of touchButtons)ports[0][id]=1;
  for(let port=0;port<4;port++){
    const pad=pads[port];if(!pad||pad.mapping!=='standard')continue;
    const left=radialStick(pad.axes[0],pad.axes[1],profile.analog_deadzone_percent),right=radialStick(pad.axes[2],pad.axes[3],profile.analog_deadzone_percent);analogPorts[port]=[...left,...right];
    for(let id=0;id<16;id++){const source=profile.button_sources[id];if(pad.buttons[PHYSICAL_BUTTONS[source]]?.pressed)ports[port][id]=1;if(!analogMode&&!(port===0&&virtualSticks[0]?.active)&&((source===6&&left[0]<-13107)||(source===7&&left[0]>13107)||(source===4&&left[1]<-13107)||(source===5&&left[1]>13107)))ports[port][id]=1;}
  }
  if(analogMode){const pressed=index=>profile.analog_keyboard[index]!==null&&analogKeys.has(profile.analog_keyboard[index]);for(let id=0;id<2;id++){const negative=pressed(id===0?2:0),positive=pressed(id===0?3:1);if(negative||positive)analogPorts[0][id]=(Number(positive)-Number(negative))*32767;}}
  for(let index=0;index<2;index++){const stick=virtualSticks[index];if(!stick?.active)continue;const values=radialStick(stick.x,stick.y,profile.analog_deadzone_percent);analogPorts[0][index*2]=values[0];analogPorts[0][index*2+1]=values[1];if(index===0&&!analogMode){if(values[0]<-13107)ports[0][6]=1;if(values[0]>13107)ports[0][7]=1;if(values[1]<-13107)ports[0][4]=1;if(values[1]>13107)ports[0][5]=1;}}
  return{ports,analogPorts};
}
