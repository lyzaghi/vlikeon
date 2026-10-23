export const defaultTouchProfile=()=>({schema_version:1,visibility:'auto',layout:'auto',left_handed:false,size_percent:100,opacity_percent:100,custom_layout:null});
export function defaultCustomTouchLayout(){
  const positions=[[820,540],[710,420],[500,850],[500,650],[160,300],[160,580],[60,440],[260,440],[930,420],[820,290],[100,90],[900,90],[280,90],[720,90],[430,500],[570,500],[230,840],[770,840]];
  return{height:1000,controls:positions.map(([x,y],id)=>({id,x,y,size_percent:100,visible:true}))};
}
export function validateCustomTouchLayout(value){
  if(value===null)return null;
  const exact=(v,keys)=>v&&typeof v==='object'&&!Array.isArray(v)&&Object.keys(v).length===keys.length&&keys.every(k=>Object.hasOwn(v,k));
  const integer=(v,min,max)=>Number.isInteger(v)&&v>=min&&v<=max;
  if(!exact(value,['height','controls'])||!integer(value.height,400,1200)||!Array.isArray(value.controls)||value.controls.length!==18||value.controls.some(v=>!exact(v,['id','x','y','size_percent','visible'])||!integer(v.id,0,17)||!integer(v.x,0,1000)||!integer(v.y,0,1000)||!integer(v.size_percent,50,200)||typeof v.visible!=='boolean')||new Set(value.controls.map(v=>v.id)).size!==18)throw new Error('Invalid custom touch layout.');
  return{height:value.height,controls:value.controls.map(v=>({...v})).sort((a,b)=>a.id-b.id)};
}
export function validateTouchProfile(value){
  const fields=['schema_version','visibility','layout','left_handed','size_percent','opacity_percent','custom_layout'];
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length<6||Object.keys(value).length>7||Object.keys(value).some(key=>!fields.includes(key))||value.schema_version!==1||!['auto','always','hidden'].includes(value.visibility)||!['auto','buttons','sticks'].includes(value.layout)||typeof value.left_handed!=='boolean'||!Number.isInteger(value.size_percent)||value.size_percent<80||value.size_percent>140||!Number.isInteger(value.opacity_percent)||value.opacity_percent<40||value.opacity_percent>100)throw new Error('Invalid touch control settings.');
  return{schema_version:1,visibility:value.visibility,layout:value.layout,left_handed:value.left_handed,size_percent:value.size_percent,opacity_percent:value.opacity_percent,custom_layout:validateCustomTouchLayout(value.custom_layout===undefined?null:value.custom_layout)};
}
