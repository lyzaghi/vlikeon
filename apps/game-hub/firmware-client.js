import{download}from'./save-client.js';
import{checkMediaCancelled}from'./media-client.js';
import{firmwareRequirements}from'./runtime/firmware.js';
import{currentLocale}from'./i18n.js';
const required=()=>new Error(currentLocale()==='vi'?'Nhập và chọn firmware cần cho trình giả lập này trong Cài đặt trước khi mở game.':'Import and select this emulator’s required firmware in Settings before opening the game.');
export async function loadFirmware(bridge,game,core,{signal}={}){
  const requirements=firmwareRequirements(core.firmware,game.system);if(!requirements.length)return null;
  if(core.firmware_protocol!==1)throw new Error('This installed emulator does not support the required firmware transfer.');
  checkMediaCancelled(signal);const vault=await bridge.request('firmware_list');checkMediaCancelled(signal);
  if(!Array.isArray(vault)||vault.length>128)throw new Error('The local firmware vault returned an invalid inventory.');
  const files=[],identity=[],groups=new Set(),used=new Set();
  for(const requirement of requirements){
    const choices=vault.filter(item=>Array.isArray(item.selected_roles)&&item.selected_roles.includes(`${game.system}:${requirement.role}`));
    if(choices.length>1)throw new Error('The local firmware role has an ambiguous selection.');const asset=choices[0];
    if(!asset){if(requirement.optional===true||requirement.required_group)continue;throw required();}
    if(typeof asset.content_hash!=='string'||!/^[a-f0-9]{64}$/.test(asset.content_hash)||!requirement.allowed_bytes.includes(asset.byte_length))throw new Error('Selected firmware does not match the installed core policy.');
    checkMediaCancelled(signal);const opened=await bridge.request('firmware_read_start',{game_id:game.id,role:requirement.role,expected_hash:asset.content_hash});
    if(opened?.role!==requirement.role||opened.sha256!==asset.content_hash||opened.byte_length!==asset.byte_length||opened.transfer?.sha256!==asset.content_hash||opened.transfer.byte_length!==asset.byte_length){if(opened?.transfer?.transfer_id)await bridge.request('transfer_abort',{transfer_id:opened.transfer.transfer_id}).catch(()=>{});throw new Error('The selected firmware changed before it could be opened.');}
    const bytes=await download(bridge,opened.transfer,{maxBytes:524288,signal});checkMediaCancelled(signal);
    const metadata={filename:requirement.filename,role:requirement.role,sha256:asset.content_hash,byte_length:asset.byte_length};files.push({...metadata,bytes});identity.push(metadata);used.add(requirement.filename);if(requirement.required_group)groups.add(requirement.required_group);
  }
  if(requirements.some(item=>!used.has(item.filename)&&(item.required_group?!groups.has(item.required_group):item.optional!==true)))throw required();
  identity.sort((a,b)=>a.filename<b.filename?-1:a.filename>b.filename?1:0);return{system:game.system,requirements,files,identity};
}
