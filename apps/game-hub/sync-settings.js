import{currentLocale}from'./i18n.js';

function node(tag,className,text){const element=document.createElement(tag);if(className)element.className=className;if(text!==undefined)element.textContent=text;return element;}
const copy={
  vi:{title:'Đồng bộ tiến trình',description:'Lưu thêm một bản mã hóa và tiếp tục trên thiết bị khác. ROM và BIOS luôn ở trên thiết bị.',create:'Tạo tài khoản đồng bộ',recover:'Kết nối bằng khóa khôi phục',disconnect:'Ngắt kết nối thiết bị này',run:'Đồng bộ ngay',cancel:'Dừng đồng bộ',service:'Địa chỉ dịch vụ HTTPS',invite:'Mã đăng ký dịch vụ',key:'Khóa khôi phục',submit:'Tiếp tục',close:'Đóng',oneTime:'Giữ khóa khôi phục ở nơi an toàn',oneTimeDescription:'Khóa này chỉ được hiển thị khi tạo tài khoản. Dùng nó để kết nối thiết bị khác, gia hạn quyền truy cập hoặc thu hồi thiết bị. Dịch vụ không giữ khóa này.',copyKey:'Sao chép khóa',copied:'Đã sao chép khóa. Hãy cất ở nơi an toàn.',connected:'Đã kết nối',disabled:'Đồng bộ đang tắt',pending:'Đăng ký dịch vụ chưa hoàn tất. Giữ khóa khôi phục rồi thử đăng ký lại.',register:'Hoàn tất đăng ký',renew:'Gia hạn quyền thiết bị',devices:'Thiết bị đã kết nối',revoke:'Thu hồi thiết bị',delete:'Xóa tài khoản trên dịch vụ',deleteWarning:'Xóa toàn bộ bản sao được mã hóa trên dịch vụ và thu hồi quyền của mọi thiết bị? Bản lưu cục bộ vẫn được giữ. Tài khoản đã xóa không thể khôi phục bằng khóa cũ.',revokeWarning:'Thu hồi thiết bị này? Nó sẽ không thể truy cập các bản sao trên dịch vụ.',working:'Đang đồng bộ…',complete:'Đồng bộ hoàn tất',cancelled:'Đã dừng đồng bộ',unknownGames:'bản lưu đang chờ thêm game phù hợp',privacy:'Bạn có thể xuất bản lưu và chơi cục bộ khi không dùng dịch vụ. Khóa của thiết bị được bảo vệ bằng kho thông tin xác thực của hệ điều hành.',loadDevices:'Xem thiết bị',localOnly:'Chưa kết nối dịch vụ',enable:'Bật lại đồng bộ',failed:'Đồng bộ chưa hoàn tất. Bản lưu cục bộ vẫn được giữ.'},
  en:{title:'Saved progress sync',description:'Keep an encrypted backup and continue on another device. ROMs and BIOS files stay on your devices.',create:'Create a sync account',recover:'Connect with a recovery key',disconnect:'Disconnect this device',run:'Sync now',cancel:'Stop syncing',service:'HTTPS service address',invite:'Service registration code',key:'Recovery key',submit:'Continue',close:'Close',oneTime:'Keep your recovery key safe',oneTimeDescription:'This key is shown when the account is created. Use it to connect another device, renew access or revoke devices. The service does not keep this key.',copyKey:'Copy recovery key',copied:'Recovery key copied. Keep it somewhere safe.',connected:'Connected',disabled:'Synchronization is disabled',pending:'Service registration is not complete. Keep the recovery key and retry registration.',register:'Complete registration',renew:'Renew device access',devices:'Connected devices',revoke:'Revoke device',delete:'Delete the remote account',deleteWarning:'Delete every encrypted copy on the service and revoke all devices? Local saves remain available. The deleted account cannot be restored with the old key.',revokeWarning:'Revoke this device? It will lose access to the encrypted copies on the service.',working:'Synchronizing…',complete:'Synchronization complete',cancelled:'Synchronization stopped',unknownGames:'saves waiting for a matching game',privacy:'You can export saves and keep playing locally without this service. Device keys are protected by the operating-system credential vault.',loadDevices:'View devices',localOnly:'No service connected',enable:'Enable synchronization',failed:'Synchronization could not finish. Local saves remain available.'}
};
const text=key=>copy[currentLocale()]?.[key]||copy.en[key];

export class SyncSettings extends EventTarget {
  #bridge;#container;#notify;#state=null;#pollTimer=0;#job=null;#statusNode=null;#action=null;#actionTimer=0;#exportTimer=0;#actionInFlight=false;#backgroundBusy=false;#refreshRevision=0;#pendingExport=false;#pendingExportStop=false;
  constructor(bridge,container,notify){super();this.#bridge=bridge;this.#container=container;this.#notify=notify;document.addEventListener('visibilitychange',()=>{if(document.hidden){clearTimeout(this.#pollTimer);clearTimeout(this.#actionTimer);clearTimeout(this.#exportTimer);}else if(this.#job?.state==='running'||this.#state?.action_protocol===1||this.#state?.pending_export_active)void this.refresh();});}
  async refresh(){
    const revision=++this.#refreshRevision;
    try{const state=await this.#bridge.request('sync_status');if(revision!==this.#refreshRevision)return;this.#state=state;if(!this.#pendingExport&&!state.pending_export_active)this.#pendingExportStop=false;if(this.#state.job)this.#job=this.#state.job;this.#action=this.#state.action_protocol===1?this.#state.action??null:null;this.#render();if(this.#job?.state==='running'&&!document.hidden)this.#schedulePoll();this.#scheduleActionPoll();this.#scheduleExportPoll();}
    catch(error){if(revision===this.#refreshRevision){clearTimeout(this.#exportTimer);this.#container.replaceChildren(node('p','muted small',error.message));}}
  }
  #button(label,action,kind='button-secondary'){
    const button=node('button',`button ${kind}`,label);button.type='button';
    button.addEventListener('click',async()=>{button.disabled=true;try{await action();}catch(error){if(error.code!=='cancelled')this.#notify(error.message,true);}finally{button.disabled=false;}});return button;
  }
  #render(){
    const panel=node('section','panel sync-panel');panel.append(node('h2','',text('title')),node('p','muted small',text('description')));
    if(this.#state.policy==='explicit_foreground')panel.append(node('p','muted small',currentLocale()==='vi'?'Giữ Game Hub mở ở phía trước khi đồng bộ. Rời ứng dụng sẽ yêu cầu dừng; mở lại và chọn Đồng bộ ngay để tiếp tục. Tác vụ mạng đang chờ có thể cần thêm thời gian để kết thúc.':'Keep Game Hub in the foreground while syncing. Leaving requests a stop; reopen it and choose Sync now to continue. A pending network operation may take additional time to finish.'));
    if(this.#state.background?.protocol===1)this.#renderBackground(panel);
    this.#statusNode=node('p','sync-status',this.#state.connected?(this.#state.enabled?text('connected'):text('disabled')):text('localOnly'));this.#statusNode.setAttribute('role','status');panel.append(this.#statusNode);
    const actions=node('div','detail-actions'),managementAvailable=this.#state.account_management_available!==false,networkAvailable=managementAvailable&&this.#state.encrypted_sync_available!==false&&(!this.#state.connected||!Array.isArray(this.#state.service_audiences)||this.#state.service_audiences.includes(this.#state.account?.audience));
    if(!networkAvailable)panel.append(node('p','muted small',currentLocale()==='vi'?'Dịch vụ đồng bộ hiện chưa khả dụng. Bạn vẫn có thể chơi và quản lý bản lưu cục bộ.':'The synchronization service is currently unavailable. You can still play and manage local saves.'));
    if(!this.#state.connected){
      if(managementAvailable)actions.append(this.#button(text('create'),()=>this.#form('create'),'button-primary'),this.#button(text('recover'),()=>this.#form('recover')));
    }else{
      panel.append(node('p','muted small',this.#state.account.audience));
      if(!this.#state.account.registered){panel.append(node('p','muted small',text('pending')));if(networkAvailable)actions.append(this.#button(text('register'),()=>this.#form('register'),'button-primary'));}
      else{
        if(networkAvailable){if(!this.#state.enabled)actions.append(this.#button(text('enable'),async()=>{if(this.#state.consent_protocol===1)await this.#bridge.request('sync_opt_in_set',{enabled:true,generation:this.#state.credential_generation,expected_epoch:this.#state.consent_epoch});else{const settings=await this.#bridge.request('settings_get');await this.#bridge.request('settings_set',{settings:{...settings,cloud_sync_opt_in:true}});}await this.#changed();}));
        else actions.append(this.#button(text('run'),()=>this.#start(),'button-primary'));}
        if(this.#state.pending_export_protocol===1)actions.append(this.#button(currentLocale()==='vi'?'Xuất bản lưu đang chờ':'Export pending saved progress',()=>this.#exportPending()));
        if(networkAvailable)actions.append(this.#button(text('loadDevices'),()=>this.#showDevices()),this.#button(text('renew'),()=>this.#form('renew')));
      }
      actions.append(this.#button(text('disconnect'),async()=>{await this.#account('disconnect');await this.#changed();}));
      if(networkAvailable)actions.append(this.#button(text('delete'),()=>this.#form('delete'),'button-danger'));
    }
    if(this.#state.consent_protocol===1&&(this.#state.consent_enabled??this.#state.enabled)){const disable=this.#button(currentLocale()==='vi'?'Tắt đồng bộ':'Disable synchronization',async()=>{await this.#bridge.request('sync_opt_in_set',{enabled:false});await this.#changed();});disable.dataset.syncWithdraw='true';actions.append(disable);}
    if(this.#pendingExport||this.#state.pending_export_active){const vi=currentLocale()==='vi',stopping=this.#pendingExportStop||this.#state.pending_export_stop_requested===true;panel.append(node('p','muted small',stopping?(vi?'Đang dừng xuất bản lưu…':'Stopping saved-progress export…'):(vi?'Đang kiểm tra và xuất bản lưu đang chờ…':'Checking and exporting pending saved progress…')));const cancel=this.#button(vi?'Dừng xuất bản lưu':'Stop saved-progress export',async()=>{this.#pendingExportStop=true;this.#render();await this.#bridge.request('sync_pending_export_cancel',{});await this.refresh();});cancel.dataset.syncWithdraw='true';cancel.disabled=stopping;actions.append(cancel);}
    if(this.#pendingExport||this.#state.pending_export_active||this.#actionInFlight||this.#action?.state==='running'||this.#action?.result_available||this.#job?.state==='running')for(const button of actions.querySelectorAll('button'))if(button.dataset.syncWithdraw!=='true')button.disabled=true;
    const privacy=this.#state.credential_store==='origin_bound_webcrypto'?(currentLocale()==='vi'?'Bạn có thể xuất bản lưu và chơi cục bộ khi không dùng dịch vụ. Trình duyệt giữ thông tin xác thực thiết bị đã mã hóa trong dữ liệu của trang này. Xóa dữ liệu trang sẽ ngắt kết nối thiết bị.':'You can export saves and play locally without this service. The browser keeps encrypted device credentials in this site’s data. Clearing site data disconnects this device.'):text('privacy');
    this.#renderAction(panel);panel.append(actions,node('p','muted small sync-privacy',privacy));this.#container.replaceChildren(panel);
    if(this.#job)this.#renderJob(this.#job);
  }
  async #exportPending(){
    if(this.#pendingExport)return;let destination=null;this.#pendingExport=true;this.#pendingExportStop=false;this.#render();
    try{destination=await this.#bridge.request('file_select_export',{suggested_name:'OmniPlay-pending-progress.opsv'});if(this.#pendingExportStop)return;const result=await this.#bridge.request('sync_pending_export',{destination_grant_id:destination.grant_id},120000);this.#notify(currentLocale()==='vi'?(result.delivery==='browser_download_started'?'Đã bắt đầu tải bản lưu. Bạn có thể nhập lại khi cần.':'Đã xuất bản lưu. Bạn có thể nhập lại khi cần.'):(result.delivery==='browser_download_started'?'Saved-progress download started. You can import it when needed.':'Saved progress exported. You can import it when needed.'));
    }finally{if(destination)await this.#bridge.request('permission_revoke',{grant_id:destination.grant_id}).catch(()=>{});this.#pendingExport=false;this.#pendingExportStop=false;await this.refresh();}
  }
  #scheduleExportPoll(){clearTimeout(this.#exportTimer);if(this.#state?.pending_export_active&&!this.#pendingExport&&!document.hidden)this.#exportTimer=setTimeout(()=>void this.refresh(),1000);}
  #renderBackground(panel){
    const vi=currentLocale()==='vi',policy=this.#state.background,label=node('label','sync-background-setting'),checkbox=node('input');checkbox.type='checkbox';checkbox.checked=policy.opt_in===true;
    // Consent remains available to disable while a background worker is finishing.
    checkbox.disabled=this.#backgroundBusy||this.#actionInFlight||this.#action?.state==='running'||this.#action?.result_available===true||(!checkbox.checked&&(!this.#state.enabled||!this.#state.account?.registered));
    const title=vi?'Cho phép đồng bộ khi ứng dụng ở nền':'Allow background synchronization';checkbox.setAttribute('aria-label',title);label.append(checkbox,node('span','',title));panel.append(label);
    panel.append(node('p','muted small',vi?'Thiết bị tự chọn thời điểm phù hợp khi có mạng và đang mở khóa. Lịch chạy có thể bị hoãn; mở Game Hub và chọn Đồng bộ ngay khi cần tiếp tục ngay. Tắt tùy chọn sẽ yêu cầu dừng lượt đang chạy.':'Your device chooses when to sync while online and unlocked. Scheduling can be delayed; open Game Hub and choose Sync now to continue immediately. Turning this off requests a stop of the current run.'));
    if(policy.opt_in===true&&policy.eligible===true&&policy.scheduler_state!=='scheduled')panel.append(node('p','muted small',vi?'Thiết bị hiện chưa xác nhận lịch đồng bộ nền. Bạn vẫn có thể chọn Đồng bộ ngay.':'Background scheduling is not currently confirmed on this device. You can still choose Sync now.'));
    checkbox.addEventListener('change',async()=>{
      if(this.#backgroundBusy)return;this.#backgroundBusy=true;checkbox.disabled=true;++this.#refreshRevision;
      try{await this.#bridge.request('sync_background_configure',{enabled:checkbox.checked});}
      catch(error){this.#notify(error.message,true);}
      finally{this.#backgroundBusy=false;await this.#changed();}
    });
  }
  #field(label,name,{type='text',placeholder='',required=true}={}){
    const wrapper=node('label','field-label');wrapper.append(node('span','',label));const input=node('input');input.name=name;input.type=type;input.required=required;input.maxLength=type==='password'?256:512;input.autocomplete='off';input.spellcheck=false;input.placeholder=placeholder;wrapper.append(input);return wrapper;
  }
  #form(mode,deviceId){
    const dialog=node('dialog','sync-dialog'),form=node('form'),abort=new AbortController();form.autocomplete='off';
    form.append(node('h2','',text({create:'create',recover:'recover',register:'register',renew:'renew',delete:'delete',revoke:'revoke'}[mode])));
    if(mode==='delete')form.append(node('p','muted small',text('deleteWarning')));
    if(mode==='revoke')form.append(node('p','muted small',text('revokeWarning')));
    if(mode==='create'||mode==='recover'){
      const services=this.#state?.service_audiences;
      if(Array.isArray(services)&&services.length>0&&services.length<=8){const label=node('label','field-label'),select=node('select');select.name='audience';select.required=true;for(const audience of services){const option=node('option','',audience);option.value=audience;select.append(option);}label.append(node('span','',text('service')),select);form.append(label);}
      else form.append(this.#field(text('service'),'audience',{type:'url',placeholder:'https://sync.example.com'}));
    }
    if(mode==='register')form.append(this.#field(text('invite'),'registration',{type:'password'}));
    if(mode!=='create')form.append(this.#field(text('key'),'recovery',{type:'password',placeholder:'OP1-…'}));
    const actions=node('div','detail-actions'),submit=node('button','button button-primary',text('submit')),close=node('button','button button-secondary',text('close'));submit.type='submit';close.type='button';close.addEventListener('click',()=>dialog.close());actions.append(submit,close);form.append(actions);
    form.addEventListener('submit',async event=>{
      event.preventDefault();submit.disabled=true;
      const field=name=>form.elements.namedItem(name)?.value||'';
      try{
        const audience=field('audience').trim();
        let result;
        const options={signal:abort.signal};
        if(mode==='create')result=await this.#account('create',{audience},options);
        if(mode==='recover')await this.#account('recover',{audience,recovery_key:field('recovery')},options);
        if(mode==='register')await this.#account('register',{recovery_key:field('recovery'),registration_token:field('registration')},options);
        if(mode==='renew')await this.#account('renew',{recovery_key:field('recovery')},options);
        if(mode==='revoke')await this.#account('revoke_device',{recovery_key:field('recovery'),device_id:deviceId},options);
        if(mode==='delete')await this.#account('delete_account',{recovery_key:field('recovery')},options);
        dialog.close();if(result){this.#showRecovery(result.recovery_key,result.registered);result.recovery_key='';}await this.#changed();
      }catch(error){if(error.code!=='cancelled')this.#notify(error.message,true);}finally{submit.disabled=false;}
    });
    dialog.addEventListener('close',()=>{abort.abort();for(const input of form.querySelectorAll('input'))input.value='';dialog.remove();});
    dialog.append(form);document.body.append(dialog);dialog.showModal();
  }
  #showRecovery(recovery,registered){
    const dialog=node('dialog','sync-dialog'),abort=new AbortController();dialog.append(node('h2','',text('oneTime')),node('p','muted small',text('oneTimeDescription')));
    if(!registered)dialog.append(node('p','muted small',text('pending')));
    const input=node('textarea','recovery-key');input.value=recovery;input.readOnly=true;input.setAttribute('aria-label',text('key'));input.spellcheck=false;dialog.append(input);
    const actions=node('div','detail-actions'),copyButton=this.#button(text('copyKey'),async()=>{await navigator.clipboard.writeText(input.value);this.#notify(text('copied'));}),close=node('button','button button-primary',text('close'));close.addEventListener('click',()=>dialog.close());actions.append(copyButton,close);dialog.append(actions);
    window.addEventListener('pagehide',()=>dialog.close(),{signal:abort.signal});dialog.addEventListener('close',()=>{abort.abort();input.value='';recovery='';dialog.remove();});document.body.append(dialog);dialog.showModal();
  }
  async #showDevices(){
    const devices=await this.#account('devices');this.#devicesDialog(devices);
  }
  #devicesDialog(devices){
    if(!Array.isArray(devices))throw new Error('The device result is invalid.');const dialog=node('dialog','sync-dialog');dialog.append(node('h2','',text('devices')));
    for(const device of devices){const row=node('div','save-item'),label=node('div');label.append(node('p','small',device.device_id),node('small','muted',new Date(device.last_seen_at_ms).toLocaleString(currentLocale())));row.append(label);if(!device.revoked_at_ms)row.append(this.#button(text('revoke'),()=>{dialog.close();this.#form('revoke',device.device_id);},'button-danger'));dialog.append(row);}
    dialog.append(this.#button(text('close'),()=>dialog.close()));dialog.addEventListener('close',()=>dialog.remove());document.body.append(dialog);dialog.showModal();
  }
  #renderAction(panel){
    const action=this.#action;if(!action)return;const box=node('div','sync-account-action');box.setAttribute('role','status');
    const vi=currentLocale()==='vi';box.append(node('p','small',action.state==='running'?(vi?'Đang xử lý yêu cầu tài khoản…':'Processing account request…'):action.result_available?(vi?'Kết quả tài khoản đang chờ nhận.':'An account result is waiting to be collected.'):action.result_expired?(vi?'Kết quả tài khoản đã hết hạn. Kiểm tra trạng thái trước khi thử lại.':'The account result expired. Check account state before retrying.'):action.error?.message??(vi?'Yêu cầu tài khoản đã kết thúc.':'The account request has finished.')));
    if(action.kind==='create'&&(action.result_expired||['cancelled','failed'].includes(action.state)))box.append(node('p','muted small',vi?'Nếu chưa giữ khóa khôi phục và tài khoản chưa đăng ký, hãy ngắt kết nối tài khoản đang chờ rồi tạo lại.':'If you did not retain the recovery key and registration is incomplete, disconnect the pending account and create another.'));
    if(action.state==='running')box.append(this.#button(text('cancel'),async()=>{this.#action=await this.#bridge.request('sync_action_cancel',{action_id:action.id});this.#render();this.#scheduleActionPoll();}));
    else{
      if(action.result_available&&!this.#actionInFlight)box.append(this.#button(vi?'Nhận kết quả':'Collect result',async()=>{
        const value=await this.#bridge.request('sync_action_collect',{action_id:action.id});this.#action={...action,result_available:false};
        if(action.kind==='create'){try{this.#showRecovery(value.recovery_key,value.registered);}finally{value.recovery_key='';}}
        else if(action.kind==='devices')this.#devicesDialog(value);await this.#changed();
      },'button-primary'));
      if(!this.#actionInFlight)box.append(this.#button(vi?'Bỏ kết quả':'Dismiss result',async()=>{if(action.kind==='create'&&action.result_available&&!confirm(vi?'Bỏ khóa khôi phục chưa nhận? Tài khoản chưa đăng ký có thể cần ngắt kết nối và tạo lại.':'Dismiss the uncollected recovery key? An unregistered account may need to be disconnected and recreated.'))return;this.#action=await this.#bridge.request('sync_action_discard',{action_id:action.id});await this.#changed();}));
    }
    panel.append(box);
  }
  #scheduleActionPoll(){clearTimeout(this.#actionTimer);if(this.#action?.state==='running'&&!this.#actionInFlight&&!document.hidden)this.#actionTimer=setTimeout(()=>void this.#pollAction(),1000);}
  async #pollAction(){if(!this.#action)return;try{this.#action=await this.#bridge.request('sync_action_status',{action_id:this.#action.id});if(this.#action.state==='running'){this.#render();this.#scheduleActionPoll();}else await this.refresh();}catch(error){this.#notify(error.message,true);clearTimeout(this.#actionTimer);}}
  async #account(kind,params={},options={}){
    if(this.#state?.action_protocol!==1)return this.#bridge.request('sync_'+kind,Object.keys(params).length?params:undefined,120000);
    if(this.#actionInFlight)throw Object.assign(new Error('Wait for the current account request.'),{code:'busy'});
    this.#actionInFlight=true;clearTimeout(this.#actionTimer);let action=null;
    const cancelled=()=>options.signal?.aborted||document.hidden;
    try{
      if(cancelled())throw Object.assign(new Error('Account request cancelled.'),{code:'cancelled'});
      action=await this.#bridge.request('sync_action_start',{action:{kind,...params}});this.#action=action;this.#render();const until=performance.now()+115000;
      while(action.state==='running'){
        if(cancelled())throw Object.assign(new Error('Account request cancelled.'),{code:'cancelled'});
        if(performance.now()>until)throw Object.assign(new Error(currentLocale()==='vi'?'Yêu cầu vẫn đang chờ. Kiểm tra kết quả trong mục đồng bộ.':'The request is still pending. Check its result in sync settings.'),{code:'deadline_exceeded'});
        await new Promise(resolve=>setTimeout(resolve,750));if(cancelled())throw Object.assign(new Error('Account request cancelled.'),{code:'cancelled'});
        action=await this.#bridge.request('sync_action_status',{action_id:action.id});this.#action=action;
      }
      if(cancelled())throw Object.assign(new Error('Account request cancelled.'),{code:'cancelled'});
      const value=await this.#bridge.request('sync_action_collect',{action_id:action.id});this.#action={...action,result_available:false};return value;
    }finally{
      if(action?.state==='running')await this.#bridge.request('sync_action_cancel',{action_id:action.id}).catch(()=>{});
      this.#actionInFlight=false;this.#render();if(!action)void this.refresh();else this.#scheduleActionPoll();
    }
  }
  async #start(){this.#job=await this.#bridge.request('sync_run_start');await this.refresh();this.#schedulePoll();}
  #schedulePoll(){clearTimeout(this.#pollTimer);if(!document.hidden)this.#pollTimer=setTimeout(()=>void this.#poll(),1500);}
  async #poll(){
    if(!this.#job)return;
    try{this.#job=await this.#bridge.request('sync_run_status',{job_id:this.#job.id});this.#renderJob(this.#job);if(this.#job.state==='running')this.#schedulePoll();else{const error=this.#job.error;await this.#changed();if(error)this.#notify(error.message,true);}}
    catch(error){this.#notify(error.message,true);clearTimeout(this.#pollTimer);this.#job=null;}
  }
  #renderJob(job){
    if(!this.#statusNode)return;
    const labels={running:'working',complete:'complete',cancelled:'cancelled',failed:'failed'};
    const opportunityFinished=job.state==='complete'&&(job.summary.more_available||job.summary.waiting_for_game||job.summary.recovery_deferred||job.summary.cloud_recovery_pending||job.summary.cloud_recovery_failed||job.summary.cloud_recovery_cleanup_pending),label=opportunityFinished?(currentLocale()==='vi'?'Lượt đồng bộ đã kết thúc':'Sync opportunity finished'):text(labels[job.state]||'working');
    this.#statusNode.replaceChildren(node('span','',`${label} · ↑ ${job.summary.uploaded} · ↓ ${job.summary.downloaded}`));
    const transfer=job.summary.transfer;
    if(transfer&&['upload','download'].includes(transfer.direction)&&Number.isSafeInteger(transfer.completed_bytes)&&Number.isSafeInteger(transfer.total_bytes)&&transfer.total_bytes>0&&transfer.total_bytes<=512*1024*1024&&transfer.completed_bytes>=0&&transfer.completed_bytes<=transfer.total_bytes){
      const vi=currentLocale()==='vi',format=new Intl.NumberFormat(vi?'vi':'en',{maximumFractionDigits:1}),amount=value=>format.format(value/1_000_000),active=job.state==='running';
      const label=active&&transfer.direction==='upload'&&transfer.completed_bytes===transfer.total_bytes?(vi?'Đang hoàn tất bản lưu':'Finishing saved progress'):transfer.direction==='upload'?(vi?'Bản lưu gửi':'Saved progress upload'):(vi?'Bản lưu nhận':'Saved progress download');
      this.#statusNode.append(node('small','muted',`${label}: ${amount(transfer.completed_bytes)} / ${amount(transfer.total_bytes)} MB${active?'':(vi?' · Đồng bộ bản lưu chưa hoàn tất.':' · This save is not fully synchronized yet.')}`));
    }
    if(job.summary.waiting_for_game)this.#statusNode.append(node('small','muted',`${job.summary.waiting_for_game} ${text('unknownGames')}`));
    if(job.summary.more_available===true)this.#statusNode.append(node('small','muted',currentLocale()==='vi'?'Còn dữ liệu cần đồng bộ. Chọn Đồng bộ ngay để tiếp tục lượt kế tiếp.':'More data remains. Choose Sync now to continue with the next opportunity.'));
    if(Number.isSafeInteger(job.summary.recovery_deferred)&&job.summary.recovery_deferred>0)this.#statusNode.append(node('small','muted',currentLocale()==='vi'?`${job.summary.recovery_deferred} bản lưu đang chờ khôi phục. Đóng game rồi đồng bộ lại.`:`${job.summary.recovery_deferred} saves are waiting for recovery. Close the game and sync again.`));
    if(Number.isSafeInteger(job.summary.pending_copy_recovered)&&job.summary.pending_copy_recovered>0)this.#statusNode.append(node('small','muted',currentLocale()==='vi'?`${job.summary.pending_copy_recovered} bản lưu được khôi phục từ bản sao chờ gửi.`:`${job.summary.pending_copy_recovered} saves recovered from pending local copies.`));
    for(const[key,vi,en]of[['cloud_recovered','Đã khôi phục từ đồng bộ','Recovered from sync'],['cloud_recovery_intact','Bản lưu đã đầy đủ','Already intact'],['cloud_recovery_pending','Đang chờ khôi phục','Recovery pending'],['cloud_recovery_failed','Cần thử khôi phục lại','Recovery needs retry'],['cloud_recovery_cleanup_pending','Tệp tạm sẽ được dọn lại','Temporary-file cleanup will retry']])if(Number.isSafeInteger(job.summary[key])&&job.summary[key]>0)this.#statusNode.append(node('small','muted',`${currentLocale()==='vi'?vi:en}: ${job.summary[key]}`));
    if(job.state==='running')this.#statusNode.append(this.#button(text('cancel'),async()=>{await this.#bridge.request('sync_run_cancel',{job_id:job.id});}));
  }
  async #changed(){await this.refresh();this.dispatchEvent(new Event('changed'));}
}
