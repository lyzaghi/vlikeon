const NUMERIC='(?:0|[1-9][0-9]*)',PRELEASE='(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)';
const VERSION=new RegExp('^'+NUMERIC+'\\.'+NUMERIC+'\\.'+NUMERIC+'(?:-'+PRELEASE+'(?:\\.'+PRELEASE+')*)?(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?$');
export const validCoreVersion=value=>typeof value==='string'&&value.length>0&&value.length<=256&&!/[^0-9A-Za-z.+-]/.test(value)&&VERSION.test(value);
export function coreResourcePaths(core){
  if(!core||typeof core.id!=='string'||!core.id.length||core.id.length>128||/[^A-Za-z0-9_-]/.test(core.id)||!validCoreVersion(core.core_version))throw new Error('The installed core identity or save version is invalid.');
  let directory=null;const result={};
  for(const[field,suffix]of[['module','.js'],['wasm','.wasm']]){
    const value=core[field];if(typeof value!=='string'||value.length>512)throw new Error('The installed core resource path is invalid.');const parts=value.split('/');
    if(parts.length!==3||parts[0]!=='cores'||parts.some(part=>!part||part==='.'||part==='..'||/[^A-Za-z0-9_.-]/.test(part))||!parts[2].endsWith(suffix)||directory!==null&&directory!==parts[1])throw new Error('Core resources must share their own bundled directory.');
    directory=parts[1];result[field]=value;
  }return result;
}
