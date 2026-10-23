import{defaultInputProfile,validateInputProfile}from'../input-profile.js';
import{defaultTouchProfile,validateTouchProfile}from'../touch-profile.js';
import{UUID,integer,requireValue,utf8}from'./errors.js';
import{Sha256}from'./sha256.js';
import{coreOptionRuntime,defaultCoreOptions,validateCoreOptions}from'../runtime/core-options.js';
import{readSyncConsent,revokeSyncConsent}from'./sync-consent.js';
import{hardwareVideoPolicy}from'../runtime/hardware-video.js';
import{coreThreadPolicy,threadsAvailable}from'../runtime/thread-policy.js';

const DEFAULTS={schema_version:1,locale:'vi',reduce_motion:false,high_contrast:false,text_scale:1,telemetry_opt_in:false,cloud_sync_opt_in:false,ai_enabled:false,suspend_background_tabs:false,appearance:{accent:'lime',library_layout:'cards',library_density:'comfortable'}};
export const defaultSettings=()=>({...DEFAULTS,appearance:{...DEFAULTS.appearance}});
export const defaultConfig=()=>({preset:'accurate',fullscreen:false,audio_volume:.8,latency_frames:2,diagnostics_enabled:false});
export function validateConfig(value){
  const fields=['preset','fullscreen','audio_volume','latency_frames','diagnostics_enabled'];
  requireValue(value&&typeof value==='object'&&Object.keys(value).every(key=>fields.includes(key))&&['accurate','smooth','battery'].includes(value.preset)&&typeof value.fullscreen==='boolean'&&Number.isFinite(value.audio_volume)&&value.audio_volume>=0&&value.audio_volume<=1&&integer(value.latency_frames,0,8)&&typeof value.diagnostics_enabled==='boolean','invalid_request','Game settings are invalid.');
  return{preset:value.preset,fullscreen:value.fullscreen,audio_volume:value.audio_volume,latency_frames:value.latency_frames,diagnostics_enabled:value.diagnostics_enabled};
}
export function validateSettings(value){
  requireValue(value&&typeof value==='object'&&Object.keys(value).every(key=>Object.hasOwn(DEFAULTS,key))&&value.schema_version===1&&['vi','en'].includes(value.locale)&&Number.isFinite(value.text_scale)&&value.text_scale>=.8&&value.text_scale<=2&&Object.keys(DEFAULTS).filter(key=>typeof DEFAULTS[key]==='boolean').every(key=>typeof value[key]==='boolean'),'invalid_request','Application settings are invalid.');
  const raw=value.appearance===undefined?DEFAULTS.appearance:value.appearance;
  requireValue(raw&&typeof raw==='object'&&!Array.isArray(raw)&&Object.keys(raw).every(key=>Object.hasOwn(DEFAULTS.appearance,key)),'invalid_request','Appearance settings are invalid.');
  const appearance={...DEFAULTS.appearance,...raw};
  requireValue(['lime','blue','violet','amber'].includes(appearance.accent)&&['cards','list'].includes(appearance.library_layout)&&['comfortable','compact'].includes(appearance.library_density),'invalid_request','Appearance settings are invalid.');
  return{...value,appearance,telemetry_opt_in:false,cloud_sync_opt_in:false,suspend_background_tabs:false};
}
const configKey=id=>`session.${id}`;
const configHash=config=>new Sha256().update(utf8(JSON.stringify(validateConfig(config)))).hex();
export class WebPreferences {
  #store;#pending=new Map();#profile=null;#syncAvailable=false;
  constructor(store){this.#store=store;}
  configureSync(profile,available){requireValue(typeof profile==='string'&&UUID.test(profile)&&typeof available==='boolean','invalid_request','The browser synchronization preference capability is invalid.');this.#profile=profile;this.#syncAvailable=available;}
  async settings(){return this.#store.transaction(['config'],'readonly',async tx=>{const settings=validateSettings((await tx.get('config','settings'))?.value??defaultSettings());if(this.#syncAvailable)settings.cloud_sync_opt_in=(await readSyncConsent(tx,this.#profile)).enabled;return settings;});}
  async setSettings(value,{signal}={}){
    const settings=validateSettings(value),enabled=value.cloud_sync_opt_in;
    return this.#store.transaction(['config'],'readwrite',async tx=>{
      if(enabled){requireValue(this.#syncAvailable&&(await readSyncConsent(tx,this.#profile)).enabled,'permission_denied','Enable synchronization explicitly in its account settings before retaining cloud consent.');settings.cloud_sync_opt_in=true;}
      else if(this.#profile)await revokeSyncConsent(tx,this.#profile);
      await tx.put('config',{key:'settings',value:settings});return settings;
    },signal);
  }
  async input(gameId){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');return this.#store.transaction(['games','config'],'readonly',async tx=>{requireValue(await tx.get('games',gameId),'not_found','The game was not found.');return validateInputProfile((await tx.get('config',`input.${gameId}`))?.value??defaultInputProfile());});}
  async setInput(gameId,value,{signal}={}){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');const input=validateInputProfile(value);await this.#store.transaction(['games','config'],'readwrite',async tx=>{requireValue(await tx.get('games',gameId),'not_found','The game was not found.');await tx.put('config',{key:`input.${gameId}`,value:input});},signal);return input;}
  async touch(gameId){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');return this.#store.transaction(['games','config'],'readonly',async tx=>{requireValue(await tx.get('games',gameId),'not_found','The game was not found.');return validateTouchProfile((await tx.get('config',`touch.${gameId}`))?.value??defaultTouchProfile());});}
  async setTouch(gameId,value,{signal}={}){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');const touch=validateTouchProfile(value);await this.#store.transaction(['games','config'],'readwrite',async tx=>{requireValue(await tx.get('games',gameId),'not_found','The game was not found.');await tx.put('config',{key:`touch.${gameId}`,value:touch});},signal);return touch;}
  async coreOptions(gameId,runtimeId){
    requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');coreOptionRuntime(runtimeId);
    return this.#store.transaction(['games','config'],'readonly',async tx=>{requireValue(await tx.get('games',gameId),'not_found','The game was not found.');return validateCoreOptions((await tx.get('config',`core_options.${gameId}.${runtimeId}`))?.value??defaultCoreOptions());});
  }
  async coreOptionRuntimes(gameId){
    requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');const prefix=`core_options.${gameId}.`;
    return this.#store.transaction(['games','config'],'readonly',async tx=>{
      requireValue(await tx.get('games',gameId),'not_found','The game was not found.');const keys=await tx.request(tx.store('config').getAllKeys(IDBKeyRange.bound(prefix,prefix+'\uffff'),33));
      requireValue(keys.length<=32&&keys.every(key=>typeof key==='string'&&key.startsWith(prefix)),'corrupt_data','Core option profile storage needs recovery.');return keys.map(key=>coreOptionRuntime(key.slice(prefix.length)));
    });
  }
  async setCoreOptions(gameId,runtimeId,value,{signal}={}){
    requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');coreOptionRuntime(runtimeId);const options=validateCoreOptions(value),key=`core_options.${gameId}.${runtimeId}`;
    await this.#store.transaction(['games','config'],'readwrite',async tx=>{
      requireValue(await tx.get('games',gameId),'not_found','The game was not found.');
      if(!Object.keys(options.values).length){await tx.delete('config',key);return;}
      if(!await tx.get('config',key)){
        const range=prefix=>IDBKeyRange.bound(prefix,prefix+'\uffff');
        const perGame=await tx.request(tx.store('config').count(range(`core_options.${gameId}.`))),total=await tx.request(tx.store('config').count(range('core_options.')));
        requireValue(perGame<32&&total<65536,'resource_limit','Core option profile storage is full. Reset unused runtime options before adding another profile.');
      }
      await tx.put('config',{key,value:options});
    },signal);return options;
  }
  async get(gameId){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');return this.#store.transaction(['games','config'],'readonly',async tx=>{requireValue(await tx.get('games',gameId),'not_found','The game was not found.');return validateConfig((await tx.get('config',configKey(gameId)))?.value??defaultConfig());});}
  async set(gameId,value,{signal}={}){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');const config=validateConfig(value);await this.#store.transaction(['games','config'],'readwrite',async tx=>{requireValue(await tx.get('games',gameId),'not_found','The game was not found.');await tx.put('config',{key:configKey(gameId),value:config});},signal);return config;}
  async advise(gameId,topic){
    const settings=await this.settings();requireValue(settings.ai_enabled,'permission_denied','Enable local recommendations before requesting advice.');const config=await this.get(gameId),next={...config},vi=settings.locale==='vi';let message,reason,change=false;
    switch(topic){
      case'audio':next.latency_frames=Math.min(8,config.latency_frames+1);change=next.latency_frames!==config.latency_frames;reason='audio_queue';message=vi?'Tăng đệm âm thanh một mức có thể giảm tiếng ngắt quãng và tăng độ trễ. Thay đổi chỉ áp dụng ở lần chơi tiếp theo.':'Increasing the audio queue may reduce interruptions and increase latency. The change applies to the next session.';break;
      case'battery':next.preset='battery';change=config.preset!=='battery';reason='battery_presentation';message=vi?'Giới hạn hiển thị 30 FPS giữ nguyên tốc độ giả lập và âm thanh. Hiệu quả pin cần được đo trên thiết bị.':'A 30 FPS presentation limit preserves emulation and audio speed. Battery benefit needs measurement on this device.';break;
      case'stutter':reason='stutter_guidance';message=vi?'Xem frame-time trong lúc chơi và đóng tác vụ nền nặng. Số đo từ phiên chơi chưa phải phép so sánh phần cứng có kiểm soát.':'Inspect frame times during play and close heavy background tasks. Interactive session measurements are not controlled hardware comparisons.';break;
      case'controls':reason='input_guidance';message=vi?'Kết nối tay cầm rồi nhấn một nút. Đặt tiêu điểm vào màn hình game để dùng bàn phím; Escape tạm dừng.':'Connect a controller and press a button. Focus the game canvas for keyboard input; Escape pauses.';break;
      case'saves':reason='save_guidance';message=vi?'Ưu tiên bản lưu trong game. Snapshot phụ thuộc core và cấu hình. Chọn tiến trình trong lịch sử khi có nhánh xung đột và xuất bản sao ra tệp.':'Prefer in-game saves. Snapshots depend on the core and configuration. Choose progress in history when branches conflict and export a file backup.';break;
      case'privacy':reason='privacy_guidance';message=vi?'Thư viện và tiến trình nằm trong dữ liệu của trình duyệt này. Quy tắc gợi ý chạy cục bộ; nội dung không được gửi đến dịch vụ AI.':'The library and progress stay in this browser storage. Recommendations use local rules; content is not sent to an AI service.';break;
      default:requireValue(false,'invalid_request','Recommendation topic is invalid.');
    }
    const now=performance.now();for(const[id,entry]of this.#pending)if(entry.expires<=now)this.#pending.delete(id);let proposal=null;
    if(change){requireValue(this.#pending.size<32,'resource_limit','Too many recommendations are pending.');proposal={id:crypto.randomUUID(),game_id:gameId,expires_at_ms:Date.now()+300000,before_hash:configHash(config),after:validateConfig(next),reason_code:reason};this.#pending.set(proposal.id,{proposal,expires:now+300000});}
    return{source:'local_rules_v1',locale:settings.locale,message,proposal,external_model_used:false};
  }
  async accept(id,{signal}={}){
    const entry=this.#pending.get(id);requireValue(entry&&entry.expires>performance.now(),'permission_expired','Request a fresh recommendation.');const proposal=entry.proposal;
    const result=await this.#store.transaction(['config','games'],'readwrite',async tx=>{
      requireValue(entry.expires>performance.now(),'permission_expired','Request a fresh recommendation.');
      const settings=validateSettings((await tx.get('config','settings'))?.value??{...DEFAULTS});requireValue(settings.ai_enabled,'permission_denied','Local recommendations are disabled.');requireValue(await tx.get('games',proposal.game_id),'not_found','The game was not found.');
      const current=validateConfig((await tx.get('config',configKey(proposal.game_id)))?.value??defaultConfig());requireValue(configHash(current)===proposal.before_hash,'conflict','Game settings changed after this recommendation.');await tx.put('config',{key:configKey(proposal.game_id),value:proposal.after});return proposal.after;
    },signal);this.#pending.delete(id);return result;
  }
  dismiss(id){this.#pending.delete(id);return{dismissed:true};}
  close(){this.#pending.clear();}
}

export function rankWebRuntime(game,device,candidates,catalog){
  requireValue(device&&typeof device.wasm==='boolean'&&typeof device.webgpu==='boolean'&&typeof device.webgl2==='boolean'&&typeof device.touch==='boolean'&&(device.thermal_throttled===null||typeof device.thermal_throttled==='boolean')&&(device.memory_mb===null||integer(device.memory_mb,1,4*1024*1024))&&(device.battery_percent===null||Number.isFinite(device.battery_percent)&&device.battery_percent>=0&&device.battery_percent<=100)&&Array.isArray(candidates)&&candidates.length<=32,'invalid_request','Runtime routing input is invalid.');
  const ids=new Set(),ranked=[];
  for(const candidate of candidates){
    requireValue(candidate&&typeof candidate.id==='string'&&/^[A-Za-z0-9_-]{1,128}$/.test(candidate.id)&&!ids.has(candidate.id),'invalid_request','Runtime routing identity is invalid.');ids.add(candidate.id);
    const core=catalog.find(core=>core.id===candidate.id),reasons=[];let eligible=true;
    if(candidate.kind!=='web_wasm'||candidate.available!==true||!core){eligible=false;reasons.push('not_installed');}
    if(core?.distribution_approved!==true){eligible=false;reasons.push('unapproved');}if(core?.security_revoked===true){eligible=false;reasons.push('revoked');}
    if(core){try{if(hardwareVideoPolicy(core)&&!device.webgl2){eligible=false;reasons.push('requires_web_gl2');}if(coreThreadPolicy(core)&&!threadsAvailable()){eligible=false;reasons.push('not_installed');}}catch{eligible=false;reasons.push('unapproved');}}
    if(!core?.systems?.includes(game.system)){eligible=false;reasons.push('wrong_system');}if(!device.wasm||typeof WebAssembly!=='object'){eligible=false;reasons.push('requires_web_assembly');}
    if(core?.needs_webgpu&&!device.webgpu){eligible=false;reasons.push('requires_web_gpu');}if(core?.minimum_memory_mb&&device.memory_mb!==null&&device.memory_mb<core.minimum_memory_mb){eligible=false;reasons.push('not_enough_memory');}
    if(eligible)reasons.push('available_local_runtime');reasons.push('unmeasured_on_device');if(device.battery_percent!==null&&device.battery_percent<20)reasons.push('battery_conservation');if(device.thermal_throttled===true)reasons.push('thermal_pressure');
    ranked.push({id:candidate.id,kind:candidate.kind,eligible,score:100,reasons});
  }
  ranked.sort((a,b)=>Number(b.eligible)-Number(a.eligible)||(a.id<b.id?-1:a.id>b.id?1:0));return{recommended_id:ranked.find(value=>value.eligible)?.id??null,candidates:ranked,automatic_mid_session_switch:false};
}
