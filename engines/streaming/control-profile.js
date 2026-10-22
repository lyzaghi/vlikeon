import{requireValue,exactKeys}from'./crypto.js';

export const REMOTE_KEYS=Object.freeze(['ArrowUp','ArrowDown','ArrowLeft','ArrowRight','Space','Enter','Tab','Escape','Backspace','ShiftLeft','ShiftRight',
  ...Array.from({length:26},(_,n)=>'Key'+String.fromCharCode(65+n)),...Array.from({length:10},(_,n)=>'Digit'+n)]);
const keys=new Set(REMOTE_KEYS);
export const REMOTE_ACTIONS=Object.freeze(['mouse_primary','mouse_secondary','mouse_middle',...REMOTE_KEYS]);
const actions=new Set(REMOTE_ACTIONS);
export const defaultRemoteControls=()=>({schema_version:1,move_keys:['KeyW','KeyS','KeyA','KeyD'],
  actions:['mouse_primary','mouse_secondary','Space','KeyE','KeyR','ShiftLeft','Escape','Enter'],
  look_sensitivity_percent:100,invert_y:false,left_handed:false,size_percent:100,opacity_percent:85});
export function validateRemoteControls(value){
  requireValue(exactKeys(value,['schema_version','move_keys','actions','look_sensitivity_percent','invert_y','left_handed','size_percent','opacity_percent'])&&
    value.schema_version===1&&Array.isArray(value.move_keys)&&value.move_keys.length===4&&new Set(value.move_keys).size===4&&value.move_keys.every(key=>keys.has(key))&&
    Array.isArray(value.actions)&&value.actions.length===8&&value.actions.every(action=>actions.has(action))&&
    Number.isInteger(value.look_sensitivity_percent)&&value.look_sensitivity_percent>=25&&value.look_sensitivity_percent<=300&&
    typeof value.invert_y==='boolean'&&typeof value.left_handed==='boolean'&&
    Number.isInteger(value.size_percent)&&value.size_percent>=80&&value.size_percent<=140&&
    Number.isInteger(value.opacity_percent)&&value.opacity_percent>=40&&value.opacity_percent<=100,
    'invalid_request','Remote touch controls exceed their supported input profile.');
  return{schema_version:1,move_keys:[...value.move_keys],actions:[...value.actions],look_sensitivity_percent:value.look_sensitivity_percent,
    invert_y:value.invert_y,left_handed:value.left_handed,size_percent:value.size_percent,opacity_percent:value.opacity_percent};
}
export function controlAction(value){
  requireValue(actions.has(value),'invalid_request','The remote action is unsupported.');
  return value==='mouse_primary'?{keys:[],buttons:1}:value==='mouse_secondary'?{keys:[],buttons:2}:value==='mouse_middle'?{keys:[],buttons:4}:{keys:[value],buttons:0};
}
