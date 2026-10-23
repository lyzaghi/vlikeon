import{GamepadPorts}from'./gamepad-ports.js';
import{GamepadHaptics}from'./gamepad-haptics.js';
import{defaultInputProfile,validateInputProfile}from'./input-profile.js';
import{gameInput}from'./game-input.js';
import{TouchControls}from'./touch-controls.js';
import{CanvasPointer}from'./canvas-pointer.js';
import{download,uploadSave,sha256,exportLiveSave}from'./save-client.js';
import{t,currentLocale}from'./i18n.js';
import{WebGameRuntime}from'./runtime/runtime.js';
import{BenchmarkCapture}from'./runtime/benchmark.js';
import{metadataCompatible}from'./compatibility.js';
import{gameMedia,coreSupportsMedia}from'./runtime/media-manifest.js';
import{checkMediaCancelled,downloadGameRom,streamGameMedia}from'./media-client.js';
import{nativeMediaAvailable,openNativeMedia}from'./native-media-client.js';
import{loadFirmware}from'./firmware-client.js';
import{webCoreOptionPolicy,resolveCoreOptions}from'./runtime/core-options.js';
import{coreResourcePaths}from'./runtime/core-resources.js';
import{libretroResultProtocol}from'./runtime/libretro-calls.js';
import{coreThreadPolicy,threadsAvailable}from'./runtime/thread-policy.js';
import{loadCoreAssets}from'./runtime/core-assets.js';
import{webPortableFilesPolicy}from'./runtime/md-portable.js';
import{coreMemoryFloor}from'./runtime/memory-policy.js';
import{hardwareVideoPolicy,hardwareVideoAvailable}from'./runtime/hardware-video.js';

function el(tag,className,text){const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;}
const labels={vi:{loading:'Đang chuẩn bị phiên chơi…',pause:'Tạm dừng',resume:'Tiếp tục',save:'Lưu nhanh',load:'Tải bản lưu gần nhất',exit:'Thoát game',fullscreen:'Toàn màn hình',autoSave:'Tự lưu tiến trình trong game mỗi 30 giây khi được hỗ trợ.',saved:'Đã lưu',error:'Phiên chơi cần được xử lý',closeWithoutSave:'Không tạo được bản lưu mới. Thoát và giữ nguyên các bản lưu trước đó?',notInstalled:'Hệ máy này chưa có core đã được cài trong bản build hiện tại.',noSnapshot:'Chưa có snapshot tương thích.',unsupportedSave:'Game này chưa hỗ trợ lưu tiến trình.'},en:{loading:'Preparing your play session…',pause:'Pause',resume:'Resume',save:'Quick save',load:'Load latest snapshot',exit:'Exit game',fullscreen:'Fullscreen',autoSave:'Supported in-game progress is saved every 30 seconds.',saved:'Saved',error:'The play session needs attention',closeWithoutSave:'A new save could not be created. Exit and keep all previous saves?',notInstalled:'No installed core supports this system in the current build.',noSnapshot:'No compatible snapshot exists yet.',unsupportedSave:'This game does not support saving progress.'}};
const text=key=>labels[currentLocale()]?.[key]||labels.en[key];
const hasInGame=info=>typeof info?.inGameSupported==='boolean'?info.inGameSupported:Boolean(info&&(info.saveRamBytes||info.saveRtcBytes||info.saveFilesSupported===true));

export class GamePlayer extends EventTarget{
  #bridge;#navigation;#runtime=null;#dialog=null;#game=null;#core=null;#info=null;#identity=null;#configHash='';#heads=new Map();#keys=new Set();#touch=null;#frame=0;#autoSave=0;#saving=false;#exiting=false;#dirty=false;#lastPauseButton=false;#status=null;#pauseButton=null;
  #ports=new GamepadPorts();#haptics=new GamepadHaptics(this.#ports,()=>this.#runtime?.state==='running');
  #inputProfile=defaultInputProfile();
  #analogKeys=new Set();
  #pointer=null;
  #observedSession=null;#heartbeatTimer=0;#heartbeatInFlight=false;
  #benchmark=null;#benchmarkStatus=null;#playSettings=null;
  #nativeFullscreen=false;#browserGuard=null;#exporting=false;#pauseTask=null;
  #preparation=null;
  #mediaSource=null;
  #discState=null;#discPanel=null;#discSelect=null;#discChange=null;#discHint=null;#changingDisc=false;
  #pauseRevision=0;
  #restoring=false;
  #portableFiles=null;
  constructor(bridge,navigation){super();this.#bridge=bridge;this.#navigation=navigation;window.addEventListener('blur',()=>{this.#keys.clear();this.#analogKeys.clear();this.#touch?.clear();this.#pauseFromEvent();});document.addEventListener('visibilitychange',()=>{if(document.hidden)this.#pauseFromEvent();});window.addEventListener('beforeunload',event=>{if(this.#runtime&&this.#dirty){event.preventDefault();event.returnValue='';}});window.omniplayLifecycle=Object.freeze({suspend:()=>this.suspend()});}
  async suspend(){if(!this.#runtime)return;if(this.#restoring){await this.pause();throw new Error('Progress restoration is still in progress.');}if(this.#preparation){this.#preparation.abort();await this.#runtime.stop();return;}await this.pause();const deadline=performance.now()+10000;while(this.#saving&&performance.now()<deadline)await new Promise(resolve=>setTimeout(resolve,50));if(this.#saving)throw new Error('A save is still being committed.');if(hasInGame(this.#info))await this.save('in_game');if(this.#info?.saveStateBytes)await this.save('emulator_state');}

  async open(game){
    if(this.#runtime||this.#preparation)throw new Error('Close the active game before starting another.');
    this.#portableFiles=null;
    const preparation=new AbortController();this.#preparation=preparation;
    this.#haptics.enable(false);this.#ports.clear();this.#runtime=new WebGameRuntime();
    const runtime=this.#runtime,check=()=>checkMediaCancelled(preparation.signal);
    this.#runtime.addEventListener('haptics',event=>this.#haptics.update(event.detail.values,event.detail.age_ms));
    this.#game=game;this.#info=null;this.#core=null;this.#heads.clear();this.#keys.clear();this.#analogKeys.clear();this.#touch?.clear();this.#dirty=false;this.#exiting=false;
    this.#discState=null;this.#changingDisc=false;
    this.#buildDialog();this.#dialog.showModal();this.#status.textContent=text('loading');
    try{
      await runtime.unlockAudio();check();
      const media=await gameMedia(game);check();
      // The component and native host both retain their own origin checks.
      this.#identity=await this.#bridge.request('hello');
      check();
      if(location.protocol==='chrome-extension:'){
        if(this.#browserGuard){await this.#releaseBrowserGuard();if(this.#browserGuard)throw new Error(currentLocale()==='vi'?'Đóng rồi mở lại tab Game Hub để khôi phục chế độ bộ nhớ.':'Close and reopen the Game Hub tab to restore its memory setting.');}
        this.#browserGuard=crypto.randomUUID();
        const guard=this.#browserGuard;await this.#bridge.request('browser_game_guard_acquire',{id:guard},10000);
        if(preparation.signal.aborted)await this.#bridge.request('browser_game_guard_release',{id:guard},10000).catch(()=>{});check();
      }
      const[inputProfile,touchProfile]=await Promise.all([this.#bridge.request('game_input_get',{game_id:game.id}),this.#bridge.request('game_touch_get',{game_id:game.id})]);check();this.#inputProfile=validateInputProfile(inputProfile);this.#ports.configure(this.#inputProfile.raw_gamepads);this.#touch.configure(touchProfile);this.#haptics.enable(this.#inputProfile.haptics_enabled);
      const settings=await this.#bridge.request('game_settings_get',{game_id:game.id});
      this.#playSettings=settings;
      check();const response=await fetch(new URL('./cores/catalog.json',import.meta.url),{signal:preparation.signal});if(!response.ok)throw new Error(text('notInstalled'));
      const catalog=await response.json();
      const cores=Array.isArray(catalog.cores)?catalog.cores.filter(core=>coreSupportsMedia(core,media)):[];check();
      const webGl2=cores.some(core=>hardwareVideoPolicy(core)!==null)&&hardwareVideoAvailable();
      const decision=await this.#bridge.request('runtime_rank',{game_id:game.id,device:{wasm:typeof WebAssembly==='object',webgpu:false,webgl2:webGl2,touch:navigator.maxTouchPoints>0,memory_mb:Number.isFinite(navigator.deviceMemory)?Math.round(navigator.deviceMemory*1024):null,battery_percent:null,thermal_throttled:null},candidates:cores.map(core=>({id:core.id,kind:'web_wasm',systems:core.systems,available:(hardwareVideoPolicy(core)===null||webGl2)&&(coreThreadPolicy(core)===null||threadsAvailable()),distribution_approved:core.distribution_approved===true,security_revoked:core.security_revoked===true,needs_webgpu:false,minimum_memory_mb:core.minimum_memory_mb??null,benchmark:null}))});
      check();this.#core=cores.find(core=>core.id===decision.recommended_id);
      if(!this.#core)throw new Error(text('notInstalled'));
      if(this.#core.file_backed_content_required===true){if(this.#core.media_filesystem_protocol!==1)throw new Error('This emulator requires an unavailable media filesystem.');media.staged=true;}
      const optionPolicy=webCoreOptionPolicy(this.#core),coreOptions=resolveCoreOptions(optionPolicy,await this.#bridge.request('game_core_options_get',{game_id:game.id,runtime_id:this.#core.id}));check();
      const discControlEnabled=this.#core.capabilities?.includes('disc_control')===true;
      this.#portableFiles=webPortableFilesPolicy(this.#core);
      if(discControlEnabled&&this.#core.disc_state_protocol!==1)throw new Error('This installed emulator does not support safe disc snapshots.');
      let savedFiles=null;
      if(this.#core.capabilities?.includes('save_files')){
        const threaded=coreThreadPolicy(this.#core)!==null;
        if(this.#core.saved_files_protocol!==1||this.#core.saved_files_io!==(threaded?'pthread_joined_barrier_v1':'single_threaded_barrier_v1')||typeof this.#core.wasm_threads!=='boolean')throw new Error('This installed emulator does not support safe persistent-file saves.');
        savedFiles={protocol:1,io:this.#core.saved_files_io,threads:threaded};
      }
      this.#haptics.enable(this.#inputProfile.haptics_enabled&&this.#core.capabilities?.includes('haptics'));
      const resources=coreResourcePaths(this.#core),wasmUrl=new URL(resources.wasm,location.href),moduleUrl=new URL(resources.module,location.href);
      if(wasmUrl.origin!==location.origin||moduleUrl.origin!==location.origin)throw new Error('Core resources must be bundled with this browser.');
      check();const wasmResponse=await fetch(wasmUrl,{signal:preparation.signal});if(!wasmResponse.ok)throw new Error('The installed core binary is missing.');
      const wasm=await wasmResponse.arrayBuffer();if(wasm.byteLength!==this.#core.wasm_bytes||await sha256(wasm)!==this.#core.wasm_sha256)throw new Error('The core binary failed its integrity check.');
      check();const firmware=await loadFirmware(this.#bridge,game,this.#core,{signal:preparation.signal});check();
      const systemAssets=await loadCoreAssets(this.#core,{signal:preparation.signal});check();
      check();const rom=media.staged?undefined:await downloadGameRom(this.#bridge,game,media,{signal:preparation.signal,onProgress:value=>{if(this.#status)this.#status.textContent=`${text('loading')} ${Math.round(value*100)}%`;}});check();
      if(media.staged&&nativeMediaAvailable(this.#identity)){
        const source=await openNativeMedia(this.#bridge,game,media,{signal:preparation.signal,onProgress:value=>{if(this.#status)this.#status.textContent=(currentLocale()==='vi'?'Đang xác minh dữ liệu game…':'Verifying game media…')+` ${Math.floor(value.bytes_hashed/value.total_bytes*100)}% · ${value.asset_index}/${value.asset_count}`;}});
        if(preparation.signal.aborted){await source.close();check();}this.#mediaSource=source;
      }
      this.#configHash=await sha256(new TextEncoder().encode(JSON.stringify({core:this.#core.id,wasm:this.#core.wasm_sha256,options:coreOptions,...(firmware?{firmware:firmware.identity}:{}),...(systemAssets?{system_assets:systemAssets.identity}:{}),...(coreThreadPolicy(this.#core)?{thread_pool:coreThreadPolicy(this.#core),pthread_worker:this.#core.pthread_worker_sha256}:{}),...(savedFiles?{saved_files:savedFiles}:{}),...(this.#portableFiles?{portable_files:this.#portableFiles}:{})})));
      this.#runtime.addEventListener('metrics',event=>{if(this.#status)this.#status.textContent=`p95 ${event.detail.frameTimeP95Ms.toFixed(1)} ms · ${event.detail.droppedFrames} dropped`;});
      this.#runtime.addEventListener('runtime_error',event=>{this.#haptics.stop();this.#keys.clear();this.#analogKeys.clear();this.#touch?.suspend();this.#pointer?.clear();this.#navigation.setGameplayActive(false);clearInterval(this.#heartbeatTimer);void this.#reportState('failed').catch(()=>{});this.#status.textContent=`${text('error')}: ${event.detail.message}`;this.#status.setAttribute('role','alert');});
      const extension=media.extension;
      this.#runtime.addEventListener('runtime_storage_warning',event=>this.dispatchEvent(new CustomEvent('warning',{detail:event.detail.message})));
      this.#info=await runtime.prepare({moduleUrl:moduleUrl.href,wasmBinary:wasm,rom,firmware,systemAssets,savedFiles,portableFiles:this.#portableFiles,resultProtocol:libretroResultProtocol(this.#core),threadPolicy:coreThreadPolicy(this.#core),minimumSystemRamBytes:coreMemoryFloor(this.#core),fileBackedContentRequired:this.#core.file_backed_content_required===true,hardwareVideo:hardwareVideoPolicy(this.#core),options:coreOptions,optionPolicy,discControlEnabled,expectedDiscCount:media.media.format==='m3u'?media.media.disc_entries.length:null,nativeMedia:this.#mediaSource,media:media.staged&&!this.#mediaSource?{media:media.media,system:media.system,total:media.total,content_hash:media.content_hash}:null,loadMedia:writer=>streamGameMedia(this.#bridge,game,media,writer,{signal:preparation.signal,onProgress:value=>{if(this.#status)this.#status.textContent=`${text('loading')} ${Math.floor(value.copied_bytes/value.total_bytes*100)}% · ${value.asset_index}/${value.asset_count}`;}}),signal:preparation.signal,canvas:this.#dialog.querySelector('canvas'),volume:settings.audio_volume,latencyFrames:settings.latency_frames,preset:settings.preset,extension,hapticsEnabled:this.#inputProfile.haptics_enabled&&this.#core.capabilities?.includes('haptics'),pointerEnabled:game.system==='nds'&&this.#core.capabilities?.includes('touch_controls')});check();
      this.#discState=this.#validateDisc(this.#info.discs);this.#renderDiscs();
      if(this.#info.version!==(this.#core.runtime_version_string||this.#core.core_version))throw new Error('The installed core version does not match its catalog.');
      const history=await this.#bridge.request('save_heads',{game_id:game.id});
      check();
      const candidates=history.filter(save=>save.slot===0&&metadataCompatible(save,this.#compatibility(save.kind))&&(save.kind==='in_game'?hasInGame(this.#info):this.#info.saveStateBytes)).sort((a,b)=>b.created_at_ms-a.created_at_ms);
      let restored=null;
      for(const save of candidates){check();try{await this.#loadSave(save,preparation.signal);check();restored=save;break;}catch(error){check();if(this.#runtime.state==='failed')throw error;}}
      const latest=history.filter(save=>save.slot===0).sort((a,b)=>b.created_at_ms-a.created_at_ms)[0];
      if(latest&&!restored&&!confirm(currentLocale()==='vi'?'Bản lưu hiện tại không tương thích hoặc không đọc được. Bắt đầu một nhánh mới và giữ lại các bản trước?':'Current progress is incompatible or unreadable. Start a separate branch and retain the previous saves?'))throw Object.assign(new Error('Cancelled'),{code:'cancelled'});
      if(latest&&restored&&latest.created_at_ms>restored.created_at_ms&&!confirm(currentLocale()==='vi'?'Tiếp tục từ bản tương thích cũ hơn? Các tiến trình khác vẫn được giữ.':'Continue from an older compatible save? Other progress remains in history.'))throw Object.assign(new Error('Cancelled'),{code:'cancelled'});
      // Continuing the newest head adopts the currently observed compatible slots.
      // Compare-and-swap protects progress created after this observation. Starting
      // fresh or choosing an older fallback preserves other slots as branches.
      if(restored&&latest?.id===restored.id)for(const head of candidates)this.#heads.set(head.kind,head);
      const session=await this.#bridge.request('web_session_begin',{game_id:game.id,runtime_id:this.#core.id});
      if(preparation.signal.aborted)await this.#bridge.request('web_session_heartbeat',{session_id:session.id,state:'stopped'},15000).catch(()=>{});check();this.#observedSession=session;
      await runtime.start();check();this.#navigation.setGameplayActive(true);this.#pauseButton.disabled=false;this.#pauseButton.textContent=text('pause');this.#dialog.querySelector('canvas').focus();
      await this.#reportState('running');
      check();
      this.#heartbeatTimer=setInterval(()=>void this.#heartbeat(),5000);
      this.#dialog.querySelector('[data-save]').disabled=!this.#info.saveStateBytes;
      this.#dialog.querySelector('[data-load]').disabled=!this.#info.saveStateBytes;
      for(const button of this.#dialog.querySelectorAll('[data-export-kind]'))button.disabled=button.dataset.exportKind==='in_game'?!hasInGame(this.#info):!this.#info.saveStateBytes;
      this.#status.textContent=`${this.#info.name} ${this.#info.version}`;
      this.#autoSave=setInterval(()=>{if(this.#runtime?.state==='running'&&!this.#saving&&!this.#exporting&&!this.#changingDisc&&!this.#restoring&&hasInGame(this.#info))void this.save('in_game').catch(error=>{this.#status.textContent=error.message;});},30000);
      this.#frame=requestAnimationFrame(()=>this.#pollInput());
    }catch(error){
      if(this.#runtime!==runtime)throw error;
      await this.#mediaSource?.close();this.#mediaSource=null;
      clearInterval(this.#heartbeatTimer);if(this.#observedSession)await this.#reportState('failed').catch(()=>{});this.#observedSession=null;
      this.#haptics.stop();this.#touch?.suspend();this.#pointer?.clear();await this.#runtime?.stop();await this.#releaseBrowserGuard();this.#runtime=null;this.#navigation.setGameplayActive(false);this.#status.textContent=error.code==='cancelled'?(currentLocale()==='vi'?'Đã dừng mở game.':'Game opening cancelled.'):error.message;this.#status.setAttribute('role','alert');this.#pauseButton.disabled=true;throw error;
    }finally{if(this.#preparation===preparation)this.#preparation=null;this.#renderDiscs();}
  }

  #buildDialog(){
    this.#touch?.dispose();this.#pointer?.dispose();this.#pointer=null;this.#dialog?.remove();const dialog=el('dialog','gameplay-dialog');
    const header=el('div','gameplay-header');header.append(el('h2','',this.#game.title));
    const exit=el('button','button button-secondary',text('exit'));exit.addEventListener('click',()=>this.exit().catch(error=>{this.#status.textContent=error.message;}));header.append(exit);
    const canvas=el('canvas','gameplay-canvas');canvas.width=160;canvas.height=144;canvas.tabIndex=0;canvas.setAttribute('aria-label',this.#game.title);
    if(this.#game.system==='nds')this.#pointer=new CanvasPointer(canvas,()=>this.#runtime?.videoGeometry,()=>this.#runtime?.state==='running'&&!this.#pauseTask&&!this.#exiting&&this.#core?.capabilities?.includes('touch_controls'));
    const controls=el('div','gameplay-controls');this.#pauseButton=el('button','button button-secondary',text('pause'));this.#pauseButton.disabled=true;this.#pauseButton.addEventListener('click',()=>this.togglePause().catch(error=>{this.#status.textContent=error.message;}));
    const save=el('button','button button-primary',text('save'));save.dataset.save='';save.disabled=true;save.addEventListener('click',()=>{save.disabled=true;this.save('emulator_state').catch(error=>{this.#status.textContent=error.message;}).finally(()=>{save.disabled=false;});});
    const load=el('button','button button-secondary',text('load'));load.dataset.load='';load.disabled=true;load.addEventListener('click',()=>{load.disabled=true;this.loadLatestSnapshot().catch(error=>{this.#status.textContent=error.message;}).finally(()=>{load.disabled=false;});});
    for(const[kind,vi,en]of[['in_game','Xuất tiến trình','Export progress'],['emulator_state','Xuất trạng thái','Export snapshot']]){const button=el('button','button button-secondary',currentLocale()==='vi'?vi:en);button.type='button';button.dataset.exportKind=kind;button.disabled=true;button.addEventListener('click',()=>{button.disabled=true;void this.exportCurrent(kind).catch(error=>{this.#status.textContent=error.message;}).finally(()=>{button.disabled=false;});});controls.append(button);}
    const fullscreen=el('button','button button-quiet',text('fullscreen'));fullscreen.addEventListener('click',()=>void(async()=>{try{if(globalThis.webkit?.messageHandlers?.omniplay){this.#nativeFullscreen=!this.#nativeFullscreen;await this.#bridge.request('presentation_fullscreen',{enabled:this.#nativeFullscreen});}else if(document.fullscreenElement)await document.exitFullscreen();else if(dialog.requestFullscreen&&document.fullscreenEnabled)await dialog.requestFullscreen();else dialog.classList.toggle('gameplay-fullscreen-fallback');}catch(error){this.#status.textContent=error.message;}})());controls.append(this.#pauseButton,save,load,fullscreen);
    this.#status=el('p','gameplay-status');this.#status.setAttribute('role','status');
    this.#touch=new TouchControls(this.#game.system,()=>this.#runtime?.state==='running'&&!this.#pauseTask&&!this.#exiting);const touch=this.#touch.element;
    const measure=el('button','button button-quiet',currentLocale()==='vi'?'Đo hiệu năng 2 phút':'Measure performance for 2 minutes');measure.type='button';
    this.#benchmarkStatus=el('p','muted small');this.#benchmarkStatus.setAttribute('role','status');
    measure.addEventListener('click',()=>void this.#startBenchmark(measure).catch(error=>{this.#benchmarkStatus.textContent=error.message;}));
    const exportDownloads=el('div','game-export-downloads');exportDownloads.dataset.exportDownloads='';exportDownloads.setAttribute('aria-label',currentLocale()==='vi'?'Tệp xuất đang chờ tải':'Export downloads');
    this.#discPanel=el('fieldset','detail-actions');this.#discPanel.hidden=true;this.#discPanel.append(el('legend','',currentLocale()==='vi'?'Đĩa game':'Game discs'));
    this.#discSelect=el('select');this.#discSelect.setAttribute('aria-label',currentLocale()==='vi'?'Đĩa muốn đưa vào':'Disc to insert');
    this.#discChange=el('button','button button-secondary',currentLocale()==='vi'?'Đưa đĩa vào':'Insert disc');this.#discChange.type='button';this.#discChange.addEventListener('click',()=>void this.selectDisc(Number(this.#discSelect.value)).catch(error=>{this.#status.textContent=error.message;}));
    this.#discHint=el('span','muted small');this.#discHint.setAttribute('role','status');this.#discPanel.append(this.#discSelect,this.#discChange,this.#discHint);
    controls.append(measure);dialog.append(header,canvas,controls,this.#discPanel,this.#status,exportDownloads,touch,this.#benchmarkStatus,el('p','muted small',text('autoSave')));
    dialog.addEventListener('cancel',event=>{event.preventDefault();this.#pauseFromEvent();});

    dialog.addEventListener('keydown',event=>{if(event.ctrlKey||event.altKey||event.metaKey)return;if(event.code==='Escape'){event.preventDefault();this.#pauseFromEvent();return;}if(event.target!==canvas||this.#runtime?.state!=='running'||this.#pauseTask||this.#exiting)return;const id=this.#inputProfile.keyboard.indexOf(event.code);if(id>=0){event.preventDefault();this.#keys.add(id);}if(['n64','psp'].includes(this.#game.system)&&this.#inputProfile.analog_keyboard.includes(event.code)){event.preventDefault();this.#analogKeys.add(event.code);}});
    dialog.addEventListener('keyup',event=>{const id=this.#inputProfile.keyboard.indexOf(event.code);if(id>=0)this.#keys.delete(id);this.#analogKeys.delete(event.code);});
    document.body.append(dialog);this.#dialog=dialog;
  }

  #compatibility(kind){const files=this.#core.saved_files_protocol===1&&this.#core.capabilities?.includes('save_files'),portable=kind==='in_game'?this.#portableFiles?.format:null;return{game_content_hash:this.#game.content_hash,core_id:this.#core.id,core_version:this.#core.core_version,state_format_version:kind==='in_game'?(portable??this.#core.in_game_format_version+(files?'.opwh0001':'')):this.#core.state_format_version+(this.#core.disc_state_protocol===1?'.opds0001':'')+(files?'.opwf0001':''),platform:'web-wasm32',core_config_hash:this.#configHash,portable_in_game_format:portable??null};}

  async save(kind,discOperation=false){
    if(this.#restoring)throw new Error(currentLocale()==='vi'?'Chờ khôi phục tiến trình kết thúc.':'Wait for progress restoration to finish.');
    if(this.#changingDisc&&!discOperation)throw new Error(currentLocale()==='vi'?'Chờ thao tác đổi đĩa kết thúc.':'Wait for the disc change to finish.');
    if(this.#exporting)throw new Error(currentLocale()==='vi'?'Đang xuất phiên chơi.':'Live progress is being exported.');
    if(this.#saving)throw new Error('Wait for the current save to finish.');if(!this.#runtime)throw new Error('There is no active game.');
    if(kind==='in_game'&&!hasInGame(this.#info))throw new Error(text('unsupportedSave'));
    this.#saving=true;
    try{
      const payload=await this.#runtime.save(kind),parent=this.#heads.get(kind),compatibility=this.#compatibility(kind);
      const metadata={schema_version:1,id:crypto.randomUUID(),game_id:this.#game.id,game_content_hash:this.#game.content_hash,kind,slot:parent?.slot??0,generation:(parent?.generation||0)+1,parent_id:parent?.id||null,core_id:compatibility.core_id,core_version:compatibility.core_version,state_format_version:compatibility.state_format_version,platform:compatibility.platform,core_config_hash:compatibility.core_config_hash,payload_hash:await sha256(payload),payload_bytes:payload.byteLength,created_at_ms:Date.now(),device_id:this.#identity.device_id,label:null};
      const saved=await uploadSave(this.#bridge,metadata,payload,parent?.id||null);
      this.#dirty=false;
      if(saved.conflict){clearInterval(this.#autoSave);this.#status.textContent=currentLocale()==='vi'?'Tiến trình đã được giữ thành một nhánh riêng. Chọn bản muốn tiếp tục trong lịch sử lưu.':'Your progress was preserved as a separate branch. Choose the progress to continue in save history.';}
      else{this.#heads.set(kind,saved);this.#status.textContent=`${text('saved')} · ${new Date(saved.created_at_ms).toLocaleTimeString(currentLocale())}`;}
      return saved;
    }finally{this.#saving=false;}
  }

  async #loadSave(metadata,signal){const runtime=this.#runtime;checkMediaCancelled(signal);const opened=await this.#bridge.request('save_read_start',{save_id:metadata.id,compatibility:this.#compatibility(metadata.kind)});const payload=await download(this.#bridge,opened.transfer,{signal});checkMediaCancelled(signal);await runtime.load(metadata.kind,payload);checkMediaCancelled(signal);if((metadata.kind==='emulator_state'||this.#info?.saveFilesSupported)&&this.#info?.discProtocol===1)await this.#refreshDiscs();checkMediaCancelled(signal);this.#heads.set(metadata.kind,metadata);this.#dirty=false;}
  async loadLatestSnapshot(){
    if(this.#exporting||this.#changingDisc||this.#restoring||this.#saving||this.#exiting||this.#preparation||!this.#runtime)throw new Error(currentLocale()==='vi'?'Chờ thao tác hiện tại kết thúc.':'Wait for the current operation to finish.');
    const save=this.#heads.get('emulator_state');if(!save)throw new Error(text('noSnapshot'));
    const runtime=this.#runtime,resume=runtime.state==='running';this.#restoring=true;
    try{await this.pause();const revision=this.#pauseRevision;await this.#loadSave(save);if(resume&&revision===this.#pauseRevision&&this.#runtime===runtime&&runtime.state==='paused'&&document.visibilityState==='visible'&&document.hasFocus()){await runtime.resume();await this.#reportState('running');this.#navigation.setGameplayActive(true);this.#pauseButton.textContent=text('pause');}}
    finally{this.#restoring=false;this.#renderDiscs();}
  }
  #validateDisc(value){if(!value||typeof value.supported!=='boolean'||!Number.isInteger(value.count)||value.count<0||value.count>8||!Number.isInteger(value.index)||value.index<0||typeof value.ejected!=='boolean'||value.supported&&(value.count<1||value.index>=value.count)||!value.supported&&(value.count!==0||value.index!==0||value.ejected))throw new Error('The emulator returned invalid disc information.');return value;}
  #renderDiscs(){
    if(!this.#discPanel)return;const value=this.#discState;this.#discPanel.hidden=!value?.supported;if(!value?.supported)return;
    this.#discSelect.replaceChildren();for(let index=0;index<value.count;index++){const option=el('option','',`${currentLocale()==='vi'?'Đĩa':'Disc'} ${index+1}`);option.value=String(index);this.#discSelect.append(option);}this.#discSelect.value=String(value.index);
    const disabled=this.#changingDisc||this.#restoring||Boolean(this.#preparation)||!['ready','paused','running'].includes(this.#runtime?.state);this.#discSelect.disabled=disabled;this.#discChange.disabled=disabled;this.#discHint.textContent=value.ejected?(currentLocale()==='vi'?'Khay đĩa đang mở':'The disc tray is open'):`${currentLocale()==='vi'?'Đang dùng đĩa':'Current disc'} ${value.index+1}/${value.count}`;
  }
  async #refreshDiscs(){this.#discState=this.#validateDisc(await this.#runtime.discStatus());this.#renderDiscs();}
  async selectDisc(index){
    if(this.#changingDisc||this.#restoring||this.#exporting||this.#exiting||this.#preparation||!['ready','paused','running'].includes(this.#runtime?.state)||!this.#discState?.supported||!Number.isInteger(index)||index<0||index>=this.#discState.count)throw new Error(currentLocale()==='vi'?'Chưa thể đổi đĩa lúc này.':'A disc change is not available right now.');
    const runtime=this.#runtime,resume=runtime.state==='running';this.#changingDisc=true;this.#renderDiscs();
    try{
      await this.pause();const pauseRevision=this.#pauseRevision,until=performance.now()+30000;while(this.#saving&&performance.now()<until)await new Promise(resolve=>setTimeout(resolve,40));if(this.#saving)throw new Error('Wait for the current save to finish.');
      if(hasInGame(this.#info))await this.save('in_game',true);
      this.#discState=this.#validateDisc(await runtime.selectDisc(index));this.#dirty=true;this.#status.textContent=currentLocale()==='vi'?'Đã đưa đĩa đã chọn vào.':'The selected disc is inserted.';
      if(resume&&pauseRevision===this.#pauseRevision&&document.visibilityState==='visible'&&document.hasFocus()&&this.#runtime===runtime){await runtime.resume();await this.#reportState('running');this.#navigation.setGameplayActive(true);this.#pauseButton.textContent=text('pause');}
    }catch(error){if(runtime===this.#runtime&&runtime.state==='paused')await this.#refreshDiscs().catch(()=>{});throw error;}finally{this.#changingDisc=false;this.#renderDiscs();}
  }
  async exportCurrent(kind){
    if(!this.#runtime||!this.#info||this.#exporting||this.#exiting||this.#changingDisc||this.#restoring)throw new Error(currentLocale()==='vi'?'Chưa thể bắt đầu xuất phiên chơi lúc này.':'The live export cannot start right now.');
    this.#exporting=true;let grant=null,locked=false;
    try{
      // Select first: the Web file picker needs the initial click's activation.
      grant=await this.#bridge.request('file_select_export',{suggested_name:`omniplay-live-${kind}-${crypto.randomUUID()}.opsave`},120000);
      try{await this.pause();}catch(error){if(this.#runtime?.state!=='paused')throw error;}while(this.#saving)await new Promise(resolve=>setTimeout(resolve,40));this.#saving=true;locked=true;
      const payload=await this.#runtime.save(kind),compatibility=this.#compatibility(kind);
      const metadata={schema_version:1,id:crypto.randomUUID(),game_id:this.#game.id,game_content_hash:this.#game.content_hash,kind,slot:0,generation:1,parent_id:null,core_id:compatibility.core_id,core_version:compatibility.core_version,state_format_version:compatibility.state_format_version,platform:compatibility.platform,core_config_hash:compatibility.core_config_hash,payload_hash:await sha256(payload),payload_bytes:payload.byteLength,created_at_ms:Date.now(),device_id:this.#identity.device_id,label:'Live progress export',conflict:false};
      this.#status.textContent=currentLocale()==='vi'?'Đang xuất phiên chơi đang tạm dừng…':'Exporting paused progress…';
      const result=await exportLiveSave(this.#bridge,metadata,payload,grant);
      this.#status.textContent=result.delivery==='browser_download_started'?(currentLocale()==='vi'?'Đã bắt đầu tải bản xuất. Kiểm tra tệp tải xong trước khi đóng game.':'Export download started. Check that it finishes before closing the game.'):(currentLocale()==='vi'?'Đã xuất tệp qua nơi bạn chọn. Game đang tạm dừng.':'Export completed through your selected destination. The game is paused.');
      return result;
    }finally{if(grant)await this.#bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});if(locked)this.#saving=false;this.#exporting=false;}
  }
  async pause(){
    this.#pauseRevision++;
    if(this.#pauseTask)return this.#pauseTask;
    const operation=(async()=>{this.#haptics.stop();this.#keys.clear();this.#analogKeys.clear();this.#touch?.suspend();this.#pointer?.clear();if(this.#runtime?.state!=='running')return;await this.#benchmark?.cancel('Playback paused.');await this.#runtime.pause();this.#navigation.setGameplayActive(false);if(this.#pauseButton)this.#pauseButton.textContent=text('resume');await this.#reportState('paused');})();this.#pauseTask=operation;
    try{return await operation;}finally{if(this.#pauseTask===operation)this.#pauseTask=null;}
  }
  async togglePause(){if(this.#restoring)throw new Error(currentLocale()==='vi'?'Chờ khôi phục tiến trình kết thúc.':'Wait for progress restoration to finish.');if(this.#changingDisc)throw new Error(currentLocale()==='vi'?'Chờ thao tác đổi đĩa kết thúc.':'Wait for the disc change to finish.');if(this.#exporting)throw new Error(currentLocale()==='vi'?'Chờ thao tác xuất kết thúc.':'Wait for the live export to finish.');if(!this.#runtime)return;if(this.#runtime.state==='running')await this.pause();else if(this.#runtime.state==='paused'){if(this.#browserGuard)await this.#bridge.request('browser_game_guard_acquire',{id:this.#browserGuard},10000);await this.#runtime.resume();await this.#reportState('running');this.#navigation.setGameplayActive(true);this.#pauseButton.textContent=text('pause');this.#dialog.querySelector('canvas').focus();}}

  async #reportState(state){if(this.#observedSession)await this.#bridge.request('web_session_heartbeat',{session_id:this.#observedSession.id,state},15000);}
  #pauseFromEvent(){void this.pause().catch(error=>{if(this.#status){this.#status.textContent=error.message;this.#status.setAttribute('role','alert');}});}
  async #startBenchmark(button){
    if(this.#exporting)throw new Error(currentLocale()==='vi'?'Chờ thao tác xuất kết thúc.':'Wait for the live export to finish.');
    if(!this.#runtime||this.#runtime.state!=='running')throw new Error(currentLocale()==='vi'?'Bắt đầu đo khi game đang chạy.':'Measure during an active game.');
    if(this.#benchmark&&['warming','measuring'].includes(this.#benchmark.state))throw new Error('A measurement is already running.');
    button.disabled=true;
    const identity={game_id:this.#game.id,game_content_hash:this.#game.content_hash,runtime_id:this.#core.id,runtime_version:this.#core.core_version,source_revision:this.#core.source_revision,runtime_build_hash:this.#core.wasm_sha256,core_config_hash:this.#configHash,preset:this.#playSettings.preset};
    this.#benchmark=new BenchmarkCapture(this.#runtime,identity);
    this.#benchmark.addEventListener('progress',event=>{const info=event.detail;this.#benchmarkStatus.textContent=info.state==='warming'?(currentLocale()==='vi'?'Đang làm nóng phiên chơi…':'Warming up the session…'):`${Math.floor(info.elapsedMs/1000)} / 120 s`;});
    this.#benchmark.addEventListener('cancelled',()=>{button.disabled=false;this.#benchmarkStatus.textContent=currentLocale()==='vi'?'Đã hủy phép đo chưa hoàn tất.':'Incomplete measurement cancelled.';});
    this.#benchmark.addEventListener('complete',event=>{void(async()=>{try{const summary=await this.#bridge.request('performance_run_commit',{run:event.detail},120000);this.#benchmarkStatus.textContent=`p95 ${summary.frame_time_p95_ms.toFixed(2)} ms · ${summary.frames_per_second.toFixed(1)} emulated FPS · ${summary.presented_frames_per_second.toFixed(1)} presented FPS · ${summary.audio_underruns} audio underruns`;}catch(error){this.#benchmarkStatus.textContent=error.message;}finally{button.disabled=false;}})();});
    try{await this.#benchmark.start();}catch(error){button.disabled=false;throw error;}
  }
  async #heartbeat(){
    if(this.#heartbeatInFlight||!this.#observedSession||!['running','paused'].includes(this.#runtime?.state))return;
    this.#heartbeatInFlight=true;
    try{if(this.#browserGuard)await this.#bridge.request('browser_game_guard_acquire',{id:this.#browserGuard},10000);await this.#reportState(this.#runtime.state);}
    catch(error){clearInterval(this.#heartbeatTimer);this.#haptics.stop();this.#touch?.suspend();this.#pointer?.clear();await this.#runtime?.pause().catch(()=>{});this.#navigation.setGameplayActive(false);this.#keys.clear();this.#analogKeys.clear();this.#status.textContent=error.message;this.#status.setAttribute('role','alert');if(this.#pauseButton)this.#pauseButton.textContent=text('resume');}
    finally{this.#heartbeatInFlight=false;}
  }

  async #releaseBrowserGuard(){if(!this.#browserGuard)return;const id=this.#browserGuard;try{await this.#bridge.request('browser_game_guard_release',{id},10000);this.#browserGuard=null;}catch{this.#status.textContent=currentLocale()==='vi'?'Chưa khôi phục chế độ bộ nhớ của tab. Đóng tab này sau khi lưu xong để giải phóng bảo vệ.':'The tab memory setting could not be restored. Close this tab after saving to release its protection.';this.dispatchEvent(new CustomEvent('warning',{detail:this.#status.textContent}));}}
  async exit(){
    if(this.#restoring){this.#status.textContent=currentLocale()==='vi'?'Chờ khôi phục tiến trình kết thúc trước khi thoát.':'Wait for progress restoration to finish before exiting.';return;}
    if(this.#changingDisc){this.#status.textContent=currentLocale()==='vi'?'Chờ thao tác đổi đĩa kết thúc trước khi thoát.':'Wait for the disc change to finish before exiting.';return;}
    if(this.#exporting){this.#status.textContent=currentLocale()==='vi'?'Chờ thao tác xuất kết thúc trước khi thoát.':'Wait for the export to finish before exiting.';return;}
    if(this.#exiting)return;this.#exiting=true;this.#preparation?.abort();
    try{
      try{await this.pause();}catch(error){this.#status.textContent=error.message;}
      if(this.#runtime&&hasInGame(this.#info)){try{await this.save('in_game');}catch(error){if(!confirm(`${error.message}\n\n${text('closeWithoutSave')}`))return;}}
      clearInterval(this.#autoSave);clearInterval(this.#heartbeatTimer);cancelAnimationFrame(this.#frame);this.#haptics.stop();this.#touch?.dispose();this.#touch=null;this.#pointer?.dispose();this.#pointer=null;await this.#runtime?.stop();await this.#mediaSource?.close();this.#mediaSource=null;await this.#releaseBrowserGuard();await this.#reportState('stopped').catch(()=>{});if(this.#nativeFullscreen){await this.#bridge.request('presentation_fullscreen',{enabled:false}).catch(()=>{});this.#nativeFullscreen=false;}this.#observedSession=null;this.#runtime=null;this.#navigation.setGameplayActive(false);this.#dirty=false;this.#dialog?.close();this.#dialog?.remove();this.#dialog=null;this.dispatchEvent(new Event('closed'));
    }finally{this.#exiting=false;}
  }

  #pollInput(){
    if(!this.#runtime)return;
    if(this.#runtime.state==='running'){
      if(document.hidden||!document.hasFocus()||this.#pauseTask){
        this.#haptics.stop();this.#keys.clear();this.#analogKeys.clear();this.#touch?.clear();this.#pointer?.clear();
        this.#runtime.input(Array.from({length:4},()=>Array(16).fill(0)),Array.from({length:4},()=>Array(4).fill(0)));
        this.#pauseFromEvent();this.#frame=requestAnimationFrame(()=>this.#pollInput());return;
      }
      const pads=this.#ports.update();
      const touch=this.#touch.snapshot(),{ports,analogPorts}=gameInput(pads,this.#inputProfile,this.#keys,touch.buttons,this.#analogKeys,['n64','psp'].includes(this.#game.system),touch.sticks);
      const pause=Boolean(pads[0]?.mapping==='standard'&&pads[0]?.buttons[16]?.pressed);if(pause&&!this.#lastPauseButton)this.#pauseFromEvent();this.#lastPauseButton=pause;
      this.#runtime.input(ports,analogPorts,this.#haptics.available(),this.#pointer?.snapshot());this.#dirty=true;
    }
    this.#frame=requestAnimationFrame(()=>this.#pollInput());
  }
}
