export const defaultTouchProfile=()=>({schema_version:1,visibility:'auto',layout:'auto',left_handed:false,size_percent:100,opacity_percent:100});
export function validateTouchProfile(value){
  const fields=['schema_version','visibility','layout','left_handed','size_percent','opacity_percent'];
  if(!value||typeof value!=='object'||Array.isArray(value)||Object.keys(value).length!==fields.length||Object.keys(value).some(key=>!fields.includes(key))||value.schema_version!==1||!['auto','always','hidden'].includes(value.visibility)||!['auto','buttons','sticks'].includes(value.layout)||typeof value.left_handed!=='boolean'||!Number.isInteger(value.size_percent)||value.size_percent<80||value.size_percent>140||!Number.isInteger(value.opacity_percent)||value.opacity_percent<40||value.opacity_percent>100)throw new Error('Invalid touch control settings.');
  return{schema_version:1,visibility:value.visibility,layout:value.layout,left_handed:value.left_handed,size_percent:value.size_percent,opacity_percent:value.opacity_percent};
}
