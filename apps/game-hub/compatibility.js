// Mirrors the public compatibility contract for choosing UI candidates. The broker
// repeats its own authoritative validation before any native restore or transfer.
export function metadataCompatible(save,target){
  if(!target||save.game_content_hash!==target.game_content_hash)return false;
  if(save.kind==='in_game'&&save.state_format_version==='ppsspp-files-opsf1'&&target.state_format_version==='ppsspp-files-opsf2'&&target.portable_in_game_format==='ppsspp-files-opsf2')return true;
  if(save.kind==='in_game'&&target.state_format_version==='ares-md-canonical-files-v1'&&target.portable_in_game_format==='ares-md-canonical-files-v1'&&(save.core_id==='ares-md-native'&&save.core_version==='0.2.0+ares.9408cb43d494'&&save.state_format_version==='ares-md-opsf2-canonical-files-native64'||save.core_id==='ares-md-web'&&save.core_version==='0.2.0+ares.9408cb43d494.omweb1'&&save.state_format_version==='ares-md-omweb1-canonical-ram-eeprom.opwh0001'))return true;
  if(save.state_format_version!==target.state_format_version)return false;
  if(save.kind==='in_game')return save.core_id===target.core_id||target.portable_in_game_format===save.state_format_version;
  return save.kind==='emulator_state'&&save.core_id===target.core_id&&save.core_version===target.core_version&&save.platform===target.platform&&save.core_config_hash===target.core_config_hash;
}
