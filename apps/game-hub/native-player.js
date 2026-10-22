import{currentLocale}from'./i18n.js';
import{metadataCompatible}from'./compatibility.js';
const labels={
  vi:{loading:'Đang chuẩn bị runtime native…',window:'Game mở trong cửa sổ riêng được cách ly. Chuyển sang cửa sổ đó để chơi; Escape tạm dừng, F11 toàn màn hình.',pause:'Tạm dừng',resume:'Tiếp tục',save:'Lưu nhanh',load:'Nạp snapshot hiện tại',exit:'Lưu và đóng game',failed:'Runtime native đã dừng. Các bản lưu trước đó vẫn được giữ.',saved:'Đã lưu tiến trình',branch:'Tiến trình đã được giữ thành một nhánh riêng. Chọn bản muốn tiếp tục trong lịch sử lưu.',fresh:'Bản lưu hiện tại không tương thích hoặc không đọc được với runtime này. Bắt đầu một nhánh chơi mới và giữ nguyên lịch sử cũ?',older:'Không thể dùng bản lưu mới nhất. Tiếp tục từ bản tương thích cũ hơn?',noSave:'Không tạo được đầy đủ bản lưu mới. Đóng game và giữ nguyên các bản đã có?',noSnapshot:'Chưa có snapshot tương thích.',ready:'Sẵn sàng',paused:'Game đang tạm dừng',starting:'Đang mở cửa sổ game…'},
  en:{loading:'Preparing the native runtime…',window:'The game opens in its own contained window. Switch to that window to play; Escape pauses and F11 toggles fullscreen.',pause:'Pause',resume:'Resume',save:'Quick save',load:'Load current snapshot',exit:'Save and close game',failed:'The native runtime stopped. Previous saves remain available.',saved:'Progress saved',branch:'Progress was preserved as a separate branch. Choose the progress to continue in save history.',fresh:'Current progress is incompatible or unreadable with this runtime. Start a separate branch and keep the existing history?',older:'The newest save could not be used. Continue from an older compatible save?',noSave:'A complete new save could not be created. Close the game and keep the saves already available?',noSnapshot:'No compatible snapshot is available.',ready:'Ready',paused:'Game paused',starting:'Opening the game window…'}
};
const text=k=>(labels[currentLocale()]||labels.en)[k];
const keyHelp={vi:'Bàn phím: mũi tên điều khiển D-pad; Z/X/A/S là các nút bấm; Enter là Start; Shift phải là Select; Q/W/E/R là vai và cò.',en:'Keyboard: arrows control the D-pad; Z/X/A/S are action buttons; Enter is Start; right Shift is Select; Q/W/E/R are shoulders and triggers.'};
const analogHelp={vi:'I/J/K/L điều khiển cần analog trái; D-pad vẫn dùng phím mũi tên. Bạn cũng có thể dùng tay cầm.',en:'I/J/K/L control the left analog stick; the arrows remain the D-pad. A gamepad is also supported.'};
const focusHelp=()=>currentLocale()==='vi'?'Chọn cửa sổ game OmniPlay và nhấn Escape để chơi, hoặc bấm Tiếp tục. Game đang chờ quyền lấy tiêu điểm.':'Select the OmniPlay game window and press Escape to play, or choose Resume. The game is waiting for focus.';
function node(tag,className,value){const item=document.createElement(tag);item.className=className;if(value!==undefined)item.textContent=value;return item;}

export class NativeGamePlayer extends EventTarget {
  #bridge;#navigation;#dialog=null;#status;#pause;#session=null;#heads=new Map();#pollTimer=0;#autoSave=0;#saving=null;#exiting=false;#opening=false;#polling=false;
  #notice='';#exporting=false;
  #discSelect=null;#discButton=null;#disc=null;
  #prepareJob=null;#cancelRequested=false;#cancelPreparation=null;
  #measureButton=null;#measureStatus=null;#measurementActive=false;#measurementStarting=false;
  #operation='';
  constructor(bridge,navigation){super();this.#bridge=bridge;this.#navigation=navigation;window.addEventListener('beforeunload',event=>{if(this.#session&&!['stopped','failed'].includes(this.#session.state)){event.preventDefault();event.returnValue='';}});}
  async open(game,runtime){
    if(this.#session||this.#opening)throw new Error('Close the active native game first.');this.#opening=true;this.#heads.clear();this.#notice='';this.#cancelRequested=false;this.#measurementActive=false;this.#measurementStarting=false;this.#operation='';
    this.#build(game);this.#dialog.showModal();this.#status.textContent=text('loading');
    try{
      const config=await this.#bridge.request('game_settings_get',{game_id:game.id});
      this.#prepareJob=await this.#bridge.request('session_prepare_start',{game_id:game.id,runtime_id:runtime.id,config});
      this.#session=await this.#waitPrepared();
      if(this.#cancelRequested)throw Object.assign(new Error('Cancelled'),{code:'cancelled'});
      const history=await this.#bridge.request('save_heads',{game_id:game.id});const targets={};
      for(const[kind,capability]of[['in_game','save_ram'],['emulator_state','save_state']])if(this.#session.capabilities.includes(capability))targets[kind]=await this.#bridge.request('session_save_compatibility',{session_id:this.#session.id,kind});
      for(const head of history)if(head.slot===0&&metadataCompatible(head,targets[head.kind]))this.#heads.set(head.kind,head);
      const candidates=Array.from(this.#heads.values()).sort((a,b)=>b.created_at_ms-a.created_at_ms);let restored=null;
      for(const save of candidates){try{await this.#bridge.request('save_load',{session_id:this.#session.id,save_id:save.id},120000);restored=save;break;}catch(error){if(!['incompatible_save','corrupt_data','content_mismatch'].includes(error.code))throw error;}}
      const latest=history.filter(save=>save.slot===0).sort((a,b)=>b.created_at_ms-a.created_at_ms)[0];
      if(!restored&&history.length){if(!window.confirm(text('fresh')))throw Object.assign(new Error('Cancelled'),{code:'cancelled'});this.#heads.clear();}
      if(restored&&latest&&latest.created_at_ms>restored.created_at_ms&&!window.confirm(text('older')))throw Object.assign(new Error('Cancelled'),{code:'cancelled'});
      try{this.#session=await this.#bridge.request('session_start',{session_id:this.#session.id},120000);}catch(error){if(error.code!=='permission_denied')throw error;this.#notice=focusHelp();}
      this.#navigation.setGameplayActive(this.#session.state==='running');this.#pause.disabled=false;this.#pause.textContent=text(this.#session.state==='running'?'pause':'resume');this.#status.textContent=this.#notice||text('window');
      this.#prepareJob=null;this.#cancelPreparation.hidden=true;
      this.#dialog.querySelector('[data-native-save]').disabled=!this.#session.capabilities.includes('save_state');
      this.#dialog.querySelector('[data-native-load]').disabled=!this.#heads.has('emulator_state');
      for(const button of this.#dialog.querySelectorAll('[data-native-export-kind]'))button.disabled=!this.#session.capabilities.includes(button.dataset.nativeExportKind==='in_game'?'save_ram':'save_state');
      this.#measureButton.hidden=!this.#session.capabilities.includes('performance_measurement');this.#measureButton.disabled=this.#measureButton.hidden;this.#measureStatus.hidden=this.#measureButton.hidden;
      if(this.#session.capabilities.includes('disc_control'))await this.#refreshDiscs();
      this.#pollTimer=setInterval(()=>void this.#poll(),1500);
      this.#autoSave=setInterval(()=>{if(this.#session?.state==='running'&&!this.#saving&&!this.#exporting&&!this.#operation&&!this.#measurementActive&&this.#session.capabilities.includes('save_ram'))void this.save('in_game').catch(error=>{this.#status.textContent=error.message;});},30000);
    }catch(error){if(this.#prepareJob)await this.#bridge.request('session_prepare_cancel',{job_id:this.#prepareJob.id}).catch(()=>{});this.#prepareJob=null;if(this.#session)await this.#bridge.request('session_stop',{session_id:this.#session.id},15000).catch(()=>{});this.#session=null;this.#navigation.setGameplayActive(false);this.#cancelPreparation.hidden=true;this.#status.textContent=error.code==='cancelled'?text('ready'):error.message;throw error;}
    finally{this.#opening=false;}
  }
  #button(label,action){const button=node('button','button button-secondary',label);button.type='button';button.addEventListener('click',async()=>{button.disabled=true;try{await action();}catch(error){this.#status.textContent=error.message;}finally{button.disabled=false;}});return button;}
  async #cancelPreparing(){this.#cancelRequested=true;this.#cancelPreparation.disabled=true;this.#status.textContent=currentLocale()==='vi'?'Đang dừng chuẩn bị runtime…':'Stopping runtime preparation…';if(this.#prepareJob)await this.#bridge.request('session_prepare_cancel',{job_id:this.#prepareJob.id});}
  async #waitPrepared(){
    const started=performance.now();
    for(;;){
      if(this.#cancelRequested){await this.#bridge.request('session_prepare_cancel',{job_id:this.#prepareJob.id});throw Object.assign(new Error('Cancelled'),{code:'cancelled'});}
      const result=await this.#bridge.request('session_prepare_status',{job_id:this.#prepareJob.id});
      if(result.state==='complete'&&result.session)return result.session;
      if(['failed','cancelled'].includes(result.state))throw Object.assign(new Error(result.error?.message||text('failed')),{code:result.error?.code||(result.state==='cancelled'?'cancelled':'runtime_unavailable')});
      if(performance.now()-started>300000){await this.#bridge.request('session_prepare_cancel',{job_id:this.#prepareJob.id});throw new Error('Runtime preparation exceeded its time bound.');}
      await new Promise(resolve=>setTimeout(resolve,500));
    }
  }
  #build(game){
    this.#dialog?.remove();const dialog=node('dialog','gameplay-dialog native-game-dialog');const header=node('div','gameplay-header');header.append(node('h2','',game.title),this.#button(text('exit'),()=>this.exit()));
    this.#status=node('p','gameplay-status');this.#status.setAttribute('role','status');const controls=node('div','gameplay-controls');this.#pause=this.#button(text('pause'),()=>this.togglePause());this.#pause.disabled=true;
    const save=this.#button(text('save'),()=>this.save('emulator_state'));save.dataset.nativeSave='';save.disabled=true;const load=this.#button(text('load'),()=>this.loadSnapshot());load.dataset.nativeLoad='';load.disabled=true;
    const exports=[];for(const[kind,vi,en]of[['in_game','Xuất tiến trình','Export progress'],['emulator_state','Xuất trạng thái','Export snapshot']]){const button=this.#button(currentLocale()==='vi'?vi:en,()=>this.exportCurrent(kind));button.dataset.nativeExportKind=kind;button.disabled=true;exports.push(button);}
    this.#discSelect=node('select');this.#discSelect.hidden=true;this.#discSelect.setAttribute('aria-label',currentLocale()==='vi'?'Đĩa game':'Game disc');this.#discButton=this.#button(currentLocale()==='vi'?'Đổi đĩa':'Switch disc',()=>this.#selectDisc());this.#discButton.hidden=true;
    this.#cancelPreparation=this.#button(currentLocale()==='vi'?'Dừng chuẩn bị':'Cancel preparation',()=>this.#cancelPreparing());
    this.#measureButton=this.#button(currentLocale()==='vi'?'Đo và tiếp tục chơi 2 phút':'Measure and resume for 2 minutes',()=>this.#toggleMeasurement());this.#measureButton.hidden=true;this.#measureButton.disabled=true;
    this.#measureStatus=node('p','muted small',currentLocale()==='vi'?'Phép đo gồm 10 giây làm nóng và 2 phút chơi liên tục. Tự lưu tạm dừng trong lúc đo; thao tác lưu, tải, đổi đĩa hoặc tạm dừng sẽ hủy phép đo chưa xong.':'Measurements include 10 seconds of warm-up and 2 minutes of uninterrupted play. Autosave waits during capture; saving, loading, disc changes or pausing cancel an incomplete run.');this.#measureStatus.setAttribute('role','status');this.#measureStatus.hidden=true;
    controls.append(this.#pause,save,load,...exports,this.#discSelect,this.#discButton,this.#cancelPreparation,this.#measureButton);dialog.append(header,node('p','muted',text('window')),node('p','muted',currentLocale()==='vi'?'Bàn phím và nguồn nút gamepad dùng cấu hình điều khiển đã lưu cho game. Sửa cấu hình trong chi tiết game trước lần mở tiếp theo.':'Keyboard and gamepad button sources use this game’s saved control mappings. Edit them in game details before the next launch.'));if(['n64','psp'].includes(game.system))dialog.append(node('p','muted',currentLocale()==='vi'?'Cần trái dùng phần gán phím analog trong cấu hình game; D-pad được giữ riêng.':'The left stick uses the game’s analog key bindings; D-pad controls stay separate.'));dialog.append(controls,this.#status,this.#measureStatus);dialog.addEventListener('cancel',event=>{event.preventDefault();void(this.#opening?this.#cancelPreparing():this.exit()).catch(error=>{this.#status.textContent=error.message;});});document.body.append(dialog);this.#dialog=dialog;
  }
  #renderMeasurement(capture,summary=null){
    this.#measurementActive=['warming','measuring'].includes(capture.state);
    this.#measureButton.textContent=this.#measurementActive?(currentLocale()==='vi'?'Hủy phép đo':'Cancel measurement'):(currentLocale()==='vi'?'Đo và tiếp tục chơi 2 phút':'Measure and resume for 2 minutes');
    if(summary)this.#measureStatus.textContent=`p95 ${summary.frame_time_p95_ms.toFixed(2)} ms · ${summary.frames_per_second.toFixed(1)} core ticks/s · ${summary.presented_frames_per_second.toFixed(1)} presented FPS · ${summary.audio_underruns} audio underruns · ${currentLocale()==='vi'?'Đã lưu số đo cục bộ.':'Measurements saved locally.'}`;
    else if(capture.state==='warming')this.#measureStatus.textContent=currentLocale()==='vi'?`Trở lại cửa sổ game. Đang làm nóng: ${Math.floor(capture.elapsed_ms/1000)} / 10 s.`:`Return to the game window. Warming up: ${Math.floor(capture.elapsed_ms/1000)} / 10 s.`;
    else if(capture.state==='measuring')this.#measureStatus.textContent=`${currentLocale()==='vi'?'Đang đo':'Measuring'}: ${Math.floor(capture.elapsed_ms/1000)} / 120 s`;
    else if(capture.state==='cancelled')this.#measureStatus.textContent=currentLocale()==='vi'?'Phép đo chưa hoàn tất đã bị hủy. Không có kết quả nào được lưu.':'The incomplete measurement was cancelled. No result was stored.';
  }
  async #toggleMeasurement(){
    if(this.#operation)throw new Error(currentLocale()==='vi'?'Chờ thao tác hiện tại kết thúc.':'Wait for the current operation to finish.');
    if(this.#exporting)throw new Error(currentLocale()==='vi'?'Chờ thao tác xuất kết thúc.':'Wait for the live export to finish.');
    if(!this.#session||this.#exiting)throw new Error('No native session is active.');
    const id=this.#session.id;
    if(this.#measurementActive){const capture=await this.#bridge.request('performance_native_cancel',{session_id:id},15000);if(this.#session?.id!==id)return;if(capture.state==='complete')await this.#pollMeasurement(id);else this.#renderMeasurement(capture);return;}
    this.#measurementActive=true;this.#measurementStarting=true;
    try{
      if(this.#saving)await this.#saving;
      if(this.#session?.id!==id||this.#exiting){if(this.#session?.id===id)this.#measurementActive=false;return;}
      const capture=await this.#bridge.request('performance_native_start',{session_id:id},15000);
      if(this.#session?.id===id&&!this.#exiting)this.#renderMeasurement(capture);
    }catch(error){await this.#bridge.request('performance_native_cancel',{session_id:id},15000).catch(()=>{});if(this.#session?.id!==id)return;this.#measurementActive=false;this.#measureButton.textContent=currentLocale()==='vi'?'Đo và tiếp tục chơi 2 phút':'Measure and resume for 2 minutes';throw error;}finally{if(this.#session?.id===id||!this.#session)this.#measurementStarting=false;}
  }
  async #pollMeasurement(id){
    const report=await this.#bridge.request('performance_native_status',{session_id:id},15000);
    if(this.#session?.id===id&&!this.#exiting)this.#renderMeasurement(report.capture,report.summary);
  }
  async save(kind,ownedOperation=false){
    if((this.#exiting||this.#opening)&&!ownedOperation)throw new Error(currentLocale()==='vi'?'Chờ phiên chơi hiện tại xử lý xong.':'Wait for the current session operation to finish.');
    if(this.#operation&&!ownedOperation)throw new Error(currentLocale()==='vi'?'Chờ thao tác hiện tại kết thúc.':'Wait for the current operation to finish.');
    if(this.#exporting)throw new Error(currentLocale()==='vi'?'Đang xuất phiên chơi.':'Live progress is being exported.');
    if(this.#saving)throw new Error(currentLocale()==='vi'?'Chờ bản lưu hiện tại hoàn tất.':'Wait for the current save to finish.');
    const operation=(async()=>{if(!this.#session)throw new Error('No native session is active.');const saved=await this.#bridge.request('save_create',{session_id:this.#session.id,kind,slot:this.#heads.get(kind)?.slot??0,expected_parent_id:this.#heads.get(kind)?.id||null},120000);
      if(saved.conflict){clearInterval(this.#autoSave);this.#notice=text('branch');this.#status.textContent=this.#notice;}else{this.#heads.set(kind,saved);this.#status.textContent=text('saved');if(kind==='emulator_state')this.#dialog.querySelector('[data-native-load]').disabled=false;}return saved;})();
    this.#saving=operation;try{return await operation;}finally{if(this.#saving===operation)this.#saving=null;}
  }
  async togglePause(){
    if(this.#operation||this.#exiting||this.#opening||this.#saving||this.#measurementStarting)throw new Error(currentLocale()==='vi'?'Chờ thao tác hiện tại kết thúc.':'Wait for the current operation to finish.');if(this.#exporting)throw new Error(currentLocale()==='vi'?'Chờ thao tác xuất kết thúc.':'Wait for the live export to finish.');if(!this.#session)return;
    const method=this.#session.state==='running'?'session_pause':this.#session.state==='ready'?'session_start':'session_resume';
    try{this.#session=await this.#bridge.request(method,{session_id:this.#session.id},15000);}catch(error){if(error.code!=='permission_denied'||method==='session_pause')throw error;this.#notice=focusHelp();this.#status.textContent=this.#notice;return;}
    if(this.#notice===focusHelp())this.#notice='';this.#navigation.setGameplayActive(this.#session.state==='running');this.#pause.textContent=text(this.#session.state==='running'?'pause':'resume');
  }
  async exportCurrent(kind){
    if(!this.#session||this.#exporting||this.#exiting||this.#operation||this.#opening||this.#measurementStarting)throw new Error(currentLocale()==='vi'?'Chưa thể bắt đầu xuất phiên chơi lúc này.':'The live export cannot start right now.');const sessionId=this.#session.id;this.#exporting=true;let grant=null;
    try{
      grant=await this.#bridge.request('file_select_export',{suggested_name:`omniplay-live-${kind}-${crypto.randomUUID()}.opsave`},120000);
      if(this.#saving)await this.#saving.catch(()=>{});if(this.#session?.id!==sessionId)throw new Error(currentLocale()==='vi'?'Phiên chơi đã thay đổi trước khi xuất.':'The game session changed before export.');
      if(this.#session.state==='running'){this.#session=await this.#bridge.request('session_pause',{session_id:sessionId},15000);this.#navigation.setGameplayActive(false);this.#pause.textContent=text('resume');}this.#status.textContent=currentLocale()==='vi'?'Đang xuất phiên chơi đang tạm dừng…':'Exporting paused progress…';
      const result=await this.#bridge.request('save_live_export',{session_id:sessionId,kind,destination_grant_id:grant.grant_id},120000);
      this.#status.textContent=currentLocale()==='vi'?'Đã xuất tệp qua nơi bạn chọn. Game đang tạm dừng.':'Export completed through your selected destination. The game is paused.';return result;
    }finally{if(grant)await this.#bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});this.#exporting=false;}
  }
  async loadSnapshot(){
    if(this.#operation||this.#exporting||this.#exiting||this.#opening||this.#saving||this.#measurementStarting)throw new Error(currentLocale()==='vi'?'Chờ thao tác hiện tại kết thúc.':'Wait for the current operation to finish.');
    const saved=this.#heads.get('emulator_state');if(!saved||!this.#session)throw new Error(text('noSnapshot'));const sessionId=this.#session.id;this.#operation='load';
    try{
      if(this.#session.state==='running')this.#session=await this.#bridge.request('session_pause',{session_id:sessionId},15000);this.#navigation.setGameplayActive(false);this.#pause.textContent=text('resume');
      await this.#bridge.request('save_load',{session_id:sessionId,save_id:saved.id},120000);if(this.#session.capabilities.includes('disc_control'))await this.#refreshDiscs();this.#status.textContent=text('paused');
    }finally{this.#operation='';}
  }
  async #refreshDiscs(){
    const value=await this.#bridge.request('session_disc_status',{session_id:this.#session.id});if(!value||typeof value.supported!=='boolean'||!Number.isInteger(value.count)||value.count<0||value.count>8||!Number.isInteger(value.index)||value.index<0||typeof value.ejected!=='boolean'||value.supported&&(value.count<1||value.index>=value.count)||!value.supported&&(value.count!==0||value.index!==0||value.ejected))throw new Error('The native disc information is invalid.');this.#disc=value;this.#discSelect.replaceChildren();
    for(let index=0;index<this.#disc.count;index++){const option=node('option','',`${currentLocale()==='vi'?'Đĩa':'Disc'} ${index+1}`);option.value=String(index);this.#discSelect.append(option);}this.#discSelect.value=String(this.#disc.index);this.#discSelect.hidden=!this.#disc.supported||this.#disc.count<2;this.#discButton.hidden=this.#discSelect.hidden;
  }
  async #selectDisc(){
    if(this.#operation||this.#exporting||this.#exiting||this.#opening||this.#saving||this.#measurementStarting)throw new Error(currentLocale()==='vi'?'Chờ thao tác hiện tại kết thúc.':'Wait for the current operation to finish.');
    if(!this.#session||!this.#disc?.supported)return;
    const index=Number(this.#discSelect.value);if(index===this.#disc.index&&!this.#disc.ejected)return;
    if(!Number.isInteger(index)||index<0||index>=this.#disc.count)throw new Error('The selected disc is invalid.');this.#operation='disc';const sessionId=this.#session.id;
    try{
      if(this.#session.state==='running')this.#session=await this.#bridge.request('session_pause',{session_id:sessionId},15000);this.#navigation.setGameplayActive(false);this.#pause.textContent=text('resume');
      if(this.#session.capabilities.includes('save_ram'))await this.save('in_game',true);
      await this.#bridge.request('session_select_disc',{session_id:sessionId,index},120000);await this.#refreshDiscs();
      this.#status.textContent=currentLocale()==='vi'?'Đã đổi đĩa. Game đang tạm dừng.':'Disc changed. The game is paused.';
    }finally{this.#operation='';}
  }
  async #poll(){
    if(!this.#session||this.#polling||this.#exiting||this.#operation)return;this.#polling=true;
    try{
      const id=this.#session.id,sessions=await this.#bridge.request('session_list');if(!this.#session||this.#session.id!==id||this.#exiting||this.#operation)return;const session=sessions.find(s=>s.id===id);if(!session)throw new Error(text('failed'));this.#session=session;
      this.#navigation.setGameplayActive(session.state==='running');this.#pause.textContent=text(session.state==='running'?'pause':'resume');
      if(session.state==='failed'||session.state==='stopped'){clearInterval(this.#pollTimer);clearInterval(this.#autoSave);this.#measurementActive=false;this.#measureButton.disabled=true;this.#status.textContent=text('failed');this.#pause.disabled=true;return;}
      if(session.close_requested){await this.exit();return;}
      if(this.#measurementActive&&!this.#measurementStarting)await this.#pollMeasurement(id);
      if(session.state==='running'){if(this.#notice===focusHelp())this.#notice='';const metrics=await this.#bridge.request('metrics_snapshot'),sample=metrics[session.id];if(sample)this.#status.textContent=`${this.#notice} p95 ${sample.frame_time_p95_ms.toFixed(2)} ms · ${sample.dropped_frames} dropped · ${sample.audio_underruns} audio underruns`;}
      else this.#status.textContent=`${this.#notice} ${text('paused')}`;
    }catch(error){this.#status.textContent=error.message;}finally{this.#polling=false;}
  }
  async exit(){
    if(this.#operation){this.#status.textContent=currentLocale()==='vi'?'Chờ thao tác hiện tại kết thúc trước khi thoát.':'Wait for the current operation to finish before exiting.';return;}
    if(this.#exporting){this.#status.textContent=currentLocale()==='vi'?'Chờ thao tác xuất kết thúc trước khi thoát.':'Wait for the export to finish before exiting.';return;}
    if(this.#exiting||this.#opening)return;this.#exiting=true;
    try{
      if(this.#saving)await this.#saving.catch(()=>{});
      if(this.#session?.state==='running'){
        try{this.#session=await this.#bridge.request('session_pause',{session_id:this.#session.id},15000);}
        catch(error){clearInterval(this.#pollTimer);this.#status.textContent=error.message;if(!window.confirm(`${error.message}\n\n${text('noSave')}`))return;await this.#bridge.request('session_stop',{session_id:this.#session.id},15000).catch(()=>{});this.#session=null;}
      }
      if(this.#session&&!['failed','stopped'].includes(this.#session.state)){
        let error=null;
        for(const[kind,cap]of[['in_game','save_ram'],['emulator_state','save_state']])if(this.#session.capabilities.includes(cap)){try{await this.save(kind,true);}catch(failure){error=failure;}}
        if(error&&!window.confirm(`${error.message}\n\n${text('noSave')}`)){this.#session=await this.#bridge.request('session_cancel_close',{session_id:this.#session.id});return;}
        await this.#bridge.request('session_stop',{session_id:this.#session.id},15000);
      }
      clearInterval(this.#pollTimer);clearInterval(this.#autoSave);this.#measurementActive=false;this.#session=null;this.#navigation.setGameplayActive(false);this.#dialog?.close();this.#dialog?.remove();this.#dialog=null;this.dispatchEvent(new Event('closed'));
    }finally{this.#exiting=false;}
  }
}
