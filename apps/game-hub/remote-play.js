import{currentLocale}from'./i18n.js';
import{openStreamingStore}from'./streaming/storage.js';
import{PairingManager}from'./streaming/pairing.js';
import{SignalTransport}from'./streaming/transport.js';
import{RemoteSession}from'./streaming/rtc.js';
import{captureAvailable,nativeWindowCaptureAvailable,displayCaptureAvailable}from'./streaming/capture.js';
import{CODEC_POLICIES,availableCodecPolicies}from'./streaming/media-policy.js';
import{RemoteInput}from'./streaming/input.js';
import{RemoteTouchControls}from'./remote-touch.js';
import{NativeInputForwarder}from'./streaming/native-input.js';
import{PROFILES}from'./streaming/qos.js';
import{requireValue,audience,fingerprint}from'./streaming/crypto.js';

const vi=()=>currentLocale()==='vi',text=(a,b)=>vi()?a:b;
function node(tag,className,value){const item=document.createElement(tag);if(className)item.className=className;if(value!==undefined)item.textContent=value;return item;}
function field(title,input){const label=node('label','field-label');label.append(node('span','',title),input);return label;}
export class RemotePlayPanel{
  #container;#notify;#store=null;#identity;#manager;#config;#status;#pairs;#requests;#offer;#joining=false;#hostOpen=false;#polling=false;#timer=0;#active=null;#transport=null;#dialog=null;#input=null;#abort=new AbortController();#busy=false;#relay;#name;#profile;#relayOnly;#inviteCode;#joinCode;#collectButton;#hostButton;
  #bridge;#canNative=false;#canCapture=false;#nativeInput=null;
  #codec;#touch=null;
  constructor(container,notify,bridge){this.#container=container;this.#notify=notify;this.#bridge=bridge;window.addEventListener('pagehide',()=>this.close(),{signal:this.#abort.signal});}
  async initialize(){
    try{
      const response=await fetch(new URL('./streaming/config.json',import.meta.url),{redirect:'error'});requireValue(response.ok,'runtime_unavailable','Remote-play configuration is unavailable.');const raw=await response.text();requireValue(raw.length<=8192,'resource_limit','Remote-play configuration exceeds its bound.');const config=JSON.parse(raw);
      requireValue(config.schema_version===1&&typeof config.enabled==='boolean'&&Array.isArray(config.relay_origins)&&config.relay_origins.length<=8&&typeof config.allow_browser_host==='boolean'&&typeof config.allow_remote_input==='boolean'&&PROFILES[config.default_profile],'invalid_request','Remote-play configuration is invalid.');for(const origin of config.relay_origins)audience(origin);this.#config=config;
      const panel=node('section','panel remote-panel'),heading=node('h2','',text('Remote play từ máy của bạn','Remote play from your own device'));this.#status=node('p','muted small');this.#status.setAttribute('role','status');panel.append(heading,this.#status);this.#container.append(panel);
      if(!config.enabled||config.relay_origins.length===0){this.#status.textContent=text('Remote play chưa được cấu hình trong bản ứng dụng này. Thư viện và tiến trình cục bộ vẫn có thể sử dụng.','Remote play is not configured in this app build. Your local library and saved progress remain available.');return;}
      if(this.#bridge){const platform=await this.#bridge.request('hello');this.#canNative=config.allow_remote_input&&platform.native_remote_input===true;this.#canCapture=platform.native_owned_capture===true;}
      this.#store=await openStreamingStore(()=>{void this.#stop();this.#status.textContent=text('Dữ liệu ghép nối đã đóng. Mở lại trang để tiếp tục.','Pairing storage closed. Reopen the page to continue.');});this.#identity=await this.#store.identity();this.#manager=new PairingManager(this.#store,this.#identity,config.relay_origins);
      this.#status.textContent=text('Ghép nối bằng lời mời riêng và so sánh mã trên cả hai thiết bị. Chỉ chia sẻ nguồn màn hình sau khi bạn chấp thuận.','Pair through a private invitation and compare the code on both devices. Screen capture starts only after your approval.');
      panel.append(node('p','muted small',text('Bản nhận diện thiết bị: ','Device fingerprint: ')+(await fingerprint(this.#identity.public_key))));
      this.#name=node('input');this.#name.maxLength=60;this.#name.value=text('Thiết bị của tôi','My device');
      this.#relay=node('select');for(const origin of config.relay_origins){const option=node('option','',origin);option.value=origin;this.#relay.append(option);}
      this.#profile=node('select');for(const[id,value]of Object.entries(PROFILES)){const option=node('option','',`${value.height}p · ${value.fps} FPS`);option.value=id;this.#profile.append(option);}this.#profile.value=config.default_profile;
      this.#codec=node('select');for(const id of availableCodecPolicies()){const option=node('option','',id==='auto'?text('Tự động','Auto'):CODEC_POLICIES[id]);option.value=id;this.#codec.append(option);}this.#codec.value='auto';panel.append(field(text('Codec video ưu tiên','Preferred video codec'),this.#codec));
      this.#relayOnly=node('input');this.#relayOnly.type='checkbox';panel.append(field(text('Tên thiết bị','Device name'),this.#name),field(text('Dịch vụ ghép nối','Signaling service'),this.#relay),field(text('Chất lượng mong muốn','Requested quality'),this.#profile),field(text('Chỉ dùng relay để hạn chế lộ địa chỉ mạng cho thiết bị kia','Use relay only to limit network-address disclosure to the peer'),this.#relayOnly));
      const pairActions=node('div','detail-actions');this.#hostButton=this.#button(text('Tạo lời mời','Create invitation'),async()=>{
        const invitation=await this.#manager.create(this.#relay.value,this.#name.value);this.#hostOpen=true;this.#inviteCode.value=invitation.code;this.#offer.hidden=false;this.#status.textContent=text('Lời mời có hiệu lực 5 phút. Gửi riêng mã này đến thiết bị muốn ghép nối.','The invitation expires in 5 minutes. Share this code privately with the device you want to pair.');this.#schedule();
      });const closeInvite=this.#button(text('Đóng lời mời','Close invitation'),async()=>{try{await this.#manager.closeInvite();}finally{this.#hostOpen=false;this.#offer.hidden=true;this.#requests.replaceChildren();}});pairActions.append(this.#hostButton,closeInvite);panel.append(pairActions);
      this.#offer=node('div','remote-invitation');this.#offer.hidden=true;this.#inviteCode=node('textarea');this.#inviteCode.readOnly=true;this.#inviteCode.rows=4;this.#inviteCode.setAttribute('aria-label',text('Mã lời mời riêng','Private invitation code'));this.#offer.append(this.#inviteCode,this.#button(text('Sao chép lời mời','Copy invitation'),async()=>{if(navigator.clipboard?.writeText)await navigator.clipboard.writeText(this.#inviteCode.value);else{this.#inviteCode.focus();this.#inviteCode.select();}}));panel.append(this.#offer);
      this.#joinCode=node('textarea');this.#joinCode.maxLength=16000;this.#joinCode.rows=3;const join=this.#button(text('Gửi yêu cầu ghép nối','Request pairing'),async()=>{
        const result=await this.#manager.join(this.#joinCode.value.trim(),this.#name.value);this.#joining=true;this.#joinCode.value='';this.#status.textContent=`${result.host_label} · ${text('Mã đối chiếu','Comparison code')}: ${result.comparison_code} · ${text('Bản nhận diện host','Host fingerprint')}: ${result.host_fingerprint}`;this.#collectButton.hidden=false;this.#schedule();
      });this.#collectButton=this.#button(text('Kiểm tra phê duyệt','Check approval'),()=>this.#collect());this.#collectButton.hidden=true;const cancelJoin=this.#button(text('Hủy yêu cầu','Cancel request'),async()=>{this.#manager.cancelJoin();this.#joining=false;this.#collectButton.hidden=true;});panel.append(field(text('Lời mời nhận từ host','Invitation from the host'),this.#joinCode),join,this.#collectButton,cancelJoin);
      this.#requests=node('div','remote-requests');this.#pairs=node('div','remote-pairs');panel.append(this.#requests,this.#pairs);await this.#refreshPairs();
    }catch(error){this.#container.replaceChildren(node('p','muted small',error.code?error.message:text('Remote play chưa sẵn sàng trên thiết bị này.','Remote play is unavailable on this device.')));}
  }
  #button(title,action){const button=node('button','button button-secondary',title);button.type='button';button.addEventListener('click',()=>void this.#task(button,action));return button;}
  async #task(button,action){if(this.#busy)return;this.#busy=true;button.disabled=true;try{await action();}catch(error){if(error.code!=='cancelled')this.#status.textContent=error.code?error.message:text('Không thể hoàn tất thao tác remote play.','The remote-play operation could not complete.');}finally{this.#busy=false;button.disabled=button.dataset.persistentDisabled==='true';}}
  #stopButton(){const button=node('button','button secondary',text('Dừng remote play','Stop remote play'));button.type='button';button.addEventListener('click',()=>void this.#stop().catch(()=>{}));return button;}
  #revokeButton(forwarder){const button=node('button','button button-secondary',text('Thu hồi điều khiển','Revoke control'));button.type='button';button.addEventListener('click',()=>void forwarder.revoke().catch(()=>{}));return button;}
  #schedule(){clearTimeout(this.#timer);if(this.#hostOpen||this.#joining)this.#timer=setTimeout(()=>void this.#poll(),2000);}
  async #poll(){if(this.#polling||this.#busy){this.#schedule();return;}this.#polling=true;
    try{if(this.#hostOpen)await this.#pending();if(this.#joining)await this.#collect();}
    catch(error){this.#status.textContent=error.message;if(['permission_expired','not_found'].includes(error.code)){this.#hostOpen=false;this.#joining=false;this.#manager.cancelJoin();this.#collectButton.hidden=true;}}
    finally{this.#polling=false;this.#schedule();}
  }
  async #pending(){
    const requests=await this.#manager.pending();
    // Preserve an approval checkbox while a poll returns the same signed request.
    const ids=requests.map(item=>item.hello.client_public_key+':'+item.document.signature).join('|');if(this.#requests.dataset.ids===ids)return;this.#requests.dataset.ids=ids;this.#requests.replaceChildren();
    for(const item of requests){
      const row=node('section','save-item remote-request'),description=node('div');description.append(node('strong','',item.hello.label),node('p','small',`${text('Mã đối chiếu','Comparison code')}: ${item.comparison_code}`),node('small','muted',item.fingerprint));
      const checked=node('input');checked.type='checkbox';const agreement=field(text('Tôi đã đối chiếu mã với thiết bị kia','I compared this code with the other device'),checked);
      const control=node('input');control.type='checkbox';control.disabled=!this.#canNative;
      const approve=this.#button(text('Phê duyệt ghép nối','Approve pairing'),async()=>{requireValue(checked.checked,'permission_denied',text('Đối chiếu mã trên cả hai thiết bị trước khi phê duyệt.','Compare the code on both devices before approving.'));await this.#manager.approve(item.document,this.#canNative&&control.checked);this.#requests.dataset.ids='';await this.#pending();await this.#refreshPairs();});
      const decline=this.#button(text('Từ chối','Decline'),async()=>{await this.#manager.decline(item.hello.client_public_key);this.#requests.dataset.ids='';await this.#pending();});description.append(agreement,field(text('Cho phép yêu cầu bàn phím/chuột; host vẫn phải duyệt từng game','Allow keyboard/mouse requests; the host must still approve each game'),control),approve,decline);row.append(description);this.#requests.append(row);
    }
  }
  async #collect(){const result=await this.#manager.collect();if(result.state==='pending')return;if(result.state==='declined'){this.#manager.cancelJoin();this.#status.textContent=text('Host đã từ chối yêu cầu.','The host declined the request.');}else{this.#status.textContent=text('Đã ghép nối. Bạn có thể yêu cầu kết nối xem.','Paired. You can request a viewing session.');await this.#refreshPairs();}this.#joining=false;this.#collectButton.hidden=true;}
  async #refreshPairs(){
    const pairs=await this.#store.list();this.#pairs.replaceChildren(node('h3','',text('Thiết bị đã ghép nối','Paired devices')));
    if(!pairs.length)this.#pairs.append(node('p','muted small',text('Chưa có thiết bị nào.','No paired devices yet.')));
    for(const pair of pairs){
      const row=node('div','save-item'),description=node('div');description.append(node('strong','',pair.label),node('small','muted',`${pair.audience} · ${pair.role==='host'?text('Thiết bị nhận','Viewer'):text('Host','Host')}`));const actions=node('div','detail-actions');
      if(pair.revoked)actions.append(this.#button(text('Gửi lại yêu cầu thu hồi','Retry relay revocation'),async()=>{await this.#manager.revoke(pair.id);await this.#refreshPairs();}));
      else if(!pair.relay_confirmed&&pair.role==='host')actions.append(this.#button(text('Gửi lại phê duyệt','Retry approval'),async()=>{await this.#manager.retryApproval(pair.id);await this.#refreshPairs();}));
      else actions.append(this.#button(pair.role==='host'?text('Chờ yêu cầu xem','Wait for a viewer'):text('Kết nối','Connect'),()=>this.#open(pair.id)));
      if(!pair.revoked)actions.append(this.#button(text('Ngắt ghép nối','Revoke pairing'),async()=>{if(!confirm(text('Thu hồi ghép nối này? Mọi phiên đang mở sẽ dừng.','Revoke this pairing? Active sessions will stop.')))return;await this.#stop();try{await this.#manager.revoke(pair.id);}finally{await this.#refreshPairs();}}));
      row.append(description,actions);this.#pairs.append(row);
    }
  }
  async #open(id){
    requireValue(!this.#active,'busy',text('Đóng phiên remote hiện tại trước.','Close the current remote session first.'));const pair=await this.#store.pair(id),transport=new SignalTransport(this.#store,this.#identity,pair,this.#config.relay_origins),session=new RemoteSession(transport,{profile:this.#profile.value,relayOnly:this.#relayOnly.checked,codec:this.#codec.value});this.#transport=transport;this.#active=session;
    const dialog=node('dialog','remote-dialog'),title=node('h2','',pair.label),status=node('p','muted small',text('Đang kết nối…','Connecting…')),video=node('video','remote-video'),display=node('div','remote-display'),actions=node('div','detail-actions'),stop=this.#stopButton();status.setAttribute('role','status');video.autoplay=true;video.playsInline=true;video.controls=false;
    const displayActions=node('div','remote-display-actions'),exitView=node('button','button button-secondary',text('Thu nhỏ','Exit expanded view'));exitView.type='button';exitView.addEventListener('click',()=>{display.classList.remove('remote-display-expanded');if(document.fullscreenElement===display)void document.exitFullscreen().catch(()=>{});});displayActions.append(exitView,this.#stopButton());display.append(video,displayActions);
    const captureChoices=node('div'),captureMode=node('select'),shareAudio=node('input'),captureStatus=node('p','muted small'),mediaStatus=node('p','muted small');shareAudio.type='checkbox';shareAudio.checked=false;captureStatus.setAttribute('role','status');
    let ownedTarget=null;
    if(this.#canCapture&&pair.role==='host'&&nativeWindowCaptureAvailable()){
      const option=node('option','',text('Cửa sổ game PC đã mở từ Vlikeon','Owned PC game window'));option.value='owned_window';captureMode.append(option);
      const games=node('select');games.setAttribute('aria-label',text('Game PC để chia sẻ','PC game to share'));
      const refresh=this.#button(text('Lấy game PC đang mở','Refresh running PC games'),async()=>{const previous=ownedTarget;ownedTarget=null;windows.replaceChildren();windows.disabled=true;captureStatus.textContent='';if(previous)await this.#bridge.request('stream_capture_release',{capture_id:previous.capture_id,session_id:previous.session_id});games.replaceChildren();const list=await this.#bridge.request('installed_game_processes');requireValue(Array.isArray(list)&&list.length<=36,'invalid_request','Installed game list is invalid.');for(const process of list.filter(value=>value.running)){const game=await this.#bridge.request('library_get',{game_id:process.game_id});if(this.#active!==session)return;if(game.system==='pc'&&![...games.options].some(option=>option.value===game.id)){const option=node('option','',game.title);option.value=game.id;games.append(option);}}});
      const windows=node('select');windows.setAttribute('aria-label',text('Cửa sổ của game đã mở','Window belonging to the running game'));windows.disabled=true;
      games.addEventListener('change',()=>{const previous=ownedTarget;ownedTarget=null;windows.replaceChildren();windows.disabled=true;captureStatus.textContent='';if(previous)void this.#bridge.request('stream_capture_release',{capture_id:previous.capture_id,session_id:previous.session_id}).catch(()=>{});});
      const prepare=this.#button(text('Lấy cửa sổ của game đã chọn','Find selected game windows'),async()=>{
        requireValue(games.value&&session.id,'invalid_state',text('Chờ yêu cầu remote rồi chọn game PC đang mở.','Wait for a remote request, then select a running PC game.'));
        if(ownedTarget)await this.#bridge.request('stream_capture_release',{capture_id:ownedTarget.capture_id,session_id:ownedTarget.session_id}).catch(()=>{});
        ownedTarget=null;windows.replaceChildren();windows.disabled=true;const gameId=games.value;
        const value=await this.#bridge.request('stream_capture_prepare',{game_id:gameId,pair_id:pair.id,session_id:session.id,host_public_key:this.#identity.public_key,grant:pair.grant});
        if(this.#active!==session||value.session_id!==session.id){await this.#bridge.request('stream_capture_release',{capture_id:value.capture_id,session_id:value.session_id}).catch(()=>{});return;}
        if(value.game_id!==gameId||value.pair_id!==pair.id||games.value!==gameId){await this.#bridge.request('stream_capture_release',{capture_id:value.capture_id,session_id:value.session_id}).catch(()=>{});return;}
        requireValue(value.selection_required===true&&Array.isArray(value.choices)&&value.choices.length>0&&value.choices.length<=32,'invalid_request','Game window choices are unavailable.');
        ownedTarget=value;const placeholder=node('option','',text('Chọn cửa sổ game…','Select a game window…'));placeholder.value='';windows.append(placeholder);
        for(const choice of value.choices){const option=node('option','',choice.title);option.value=choice.choice_id;windows.append(option);}windows.disabled=false;
        captureMode.value='owned_window';captureStatus.textContent=text('Chọn cửa sổ rồi nhấn Dùng cửa sổ này trong 30 giây.','Select a window and press Use this window within 30 seconds.');
      });
      const selectWindow=this.#button(text('Dùng cửa sổ này','Use this window'),async()=>{
        const pending=ownedTarget;requireValue(pending?.selection_required===true&&pending.game_id===games.value&&windows.value&&pending.choice_expires_at_ms>Date.now(),'invalid_state',text('Lấy lại danh sách rồi chọn cửa sổ game.','Refresh and select a game window.'));
        const value=await this.#bridge.request('stream_capture_select',{capture_id:pending.capture_id,session_id:pending.session_id,choice_id:windows.value});
        if(this.#active!==session||ownedTarget!==pending||games.value!==pending.game_id){await this.#bridge.request('stream_capture_release',{capture_id:value.capture_id,session_id:value.session_id}).catch(()=>{});return;}
        requireValue(value.capture_id===pending.capture_id&&value.session_id===session.id&&value.selection_required===false,'unauthorized','The selected capture identity changed.');
        ownedTarget=value;windows.disabled=true;captureStatus.textContent=text('Đã chọn. Nhấn Chấp thuận và chọn đúng cửa sổ này trong hộp thoại chia sẻ.','Selected. Press Approve and choose this same window in the sharing dialog.');
      });
      captureChoices.append(games,refresh,prepare,windows,selectWindow);
    }
    if(nativeWindowCaptureAvailable()){const option=node('option','',text('Chọn cửa sổ game','Choose a game window'));option.value='window';captureMode.append(option);}
    if(displayCaptureAvailable()){const option=node('option','',text('Chọn tab, cửa sổ hoặc màn hình','Choose a tab, window or display'));option.value='display';captureMode.append(option);}
    captureChoices.append(field(text('Nguồn chia sẻ','Capture source'),captureMode),field(text('Chia sẻ âm thanh được bạn cho phép trong hộp chọn nguồn','Share audio you authorize in the source picker'),shareAudio));captureChoices.hidden=true;
    const play=this.#button(text('Bật hình và âm thanh','Enable video and audio'),()=>video.play()),fullscreen=this.#button(text('Mở rộng màn hình game','Expand game view'),async()=>{if(typeof display.requestFullscreen==='function')try{await display.requestFullscreen();return;}catch{}display.classList.add('remote-display-expanded');}),capture=this.#button(text('Chấp thuận và chọn màn hình game','Approve and select the game capture'),async()=>{
      requireValue(this.#config.allow_browser_host,'permission_denied','Browser hosting is disabled.');const owned=captureMode.value==='owned_window';requireValue(!owned||ownedTarget?.session_id===session.id&&ownedTarget.selection_required===false,'invalid_state',text('Lấy danh sách và xác nhận cửa sổ game trước.','Find and confirm a game window first.'));const observed=await session.capture({source:owned?'window':captureMode.value,audio:shareAudio.checked,owned:owned?ownedTarget:null});if(this.#active!==session)return;capture.hidden=true;captureChoices.hidden=true;status.textContent=observed.audio_present?text('Đang chia sẻ hình và âm thanh bạn đã chọn.','Sharing the video and audio you selected.'):text('Đang chia sẻ hình của nguồn bạn đã chọn.','Sharing video from your selected source.');
    });capture.hidden=true;
    actions.append(play,fullscreen,capture,stop);dialog.append(title,display,status,captureChoices,captureStatus,mediaStatus,actions);document.body.append(dialog);this.#dialog=dialog;
    const controlStatus=node('p','muted small'),lockPointer=this.#button(text('Dùng chuột trên game từ xa','Use the mouse in the remote game'),async()=>this.#input?.lockPointer());lockPointer.hidden=true;actions.append(lockPointer);dialog.append(controlStatus,node('p','muted small',text('Shift + Tab đưa bàn phím trở về các nút của OmniPlay.','Shift + Tab returns keyboard focus to the OmniPlay controls.')));
    if(pair.role==='host'&&this.#canNative&&pair.input_allowed){
      const forwarder=new NativeInputForwarder(this.#bridge,session,pair,this.#identity);this.#nativeInput=forwarder;const games=node('select'),refresh=this.#button(text('Game PC đang mở','Running PC games'),async()=>{games.replaceChildren();for(const game of await forwarder.games()){const option=node('option','',game.title);option.value=game.id;games.append(option);}controlStatus.textContent=games.options.length?text('Chọn đúng game đang chia sẻ, rồi xác nhận quyền điều khiển tại host.','Choose the game being shared, then confirm control permission on the host.'):text('Mở game PC từ thư viện OmniPlay trước.','Launch a PC game from the OmniPlay library first.');}),arm=this.#button(text('Cho phép bàn phím/chuột cho game này','Authorize keyboard/mouse for this game'),async()=>{requireValue(games.value,'invalid_state',text('Chọn game PC đang mở trước.','Choose a running PC game first.'));await forwarder.arm(games.value);}),revoke=this.#revokeButton(forwarder);games.setAttribute('aria-label',text('Game PC nhận điều khiển','PC game receiving input'));arm.disabled=true;actions.append(games,refresh,arm,revoke);
      let connected=false,inputState=null;
      controlStatus.setAttribute('role','status');controlStatus.setAttribute('aria-live','polite');
      const syncControls=()=>{
        const unavailable=!connected||inputState!==null&&inputState!=='revoked';
        arm.dataset.persistentDisabled=String(unavailable);arm.disabled=unavailable;
        revoke.disabled=inputState==='revoking';
        revoke.textContent=['cleanup_required','cleanup_unknown'].includes(inputState)?text('Kiểm tra thu hồi','Check revocation'):text('Thu hồi điều khiển','Revoke control');
      };
      session.addEventListener('state',event=>{connected=event.detail.state==='connected';syncControls();});
      forwarder.addEventListener('status',event=>{
        const value=event.detail;inputState=value.state;syncControls();
        switch(value.state){
          case 'waiting_for_target':controlStatus.textContent=text('Đưa game đã chọn ra phía trước trong 30 giây. F12 tại host thu hồi điều khiển.','Bring the selected game forward within 30 seconds. Host F12 revokes control.');break;
          case 'revoking':controlStatus.textContent=text('Đã chặn gửi input. Đang chờ host xác nhận thu hồi…','Input forwarding is blocked. Waiting for the host to confirm revocation…');break;
          case 'cleanup_required':controlStatus.textContent=text('Host đang hoàn tất thu hồi. Kiểm tra lại trước khi cấp quyền mới.','The host is finishing revocation. Check again before authorizing new control.');break;
          case 'cleanup_unknown':controlStatus.textContent=text('Chưa xác nhận được thu hồi tại host. Input vẫn bị chặn; chọn Kiểm tra thu hồi để thử lại.','Host revocation is not confirmed. Input remains blocked; choose Check revocation to retry.');break;
          case 'revoked':controlStatus.textContent=value.reason==='input_timeout'?text('Luồng điều khiển đã gián đoạn. Host cần duyệt lại để tiếp tục.','Input delivery was interrupted. Host approval is needed to continue.'):text('Host đã xác nhận thu hồi. Cần duyệt lại để tiếp tục điều khiển.','The host confirmed revocation. New approval is required to resume control.');break;
          default:controlStatus.textContent=text('Bàn phím/chuột đã được cho phép cho game đang ở phía trước. F12 thu hồi quyền.','Keyboard/mouse is authorized for the foreground game. F12 revokes control.');
        }
      });forwarder.addEventListener('input_error',event=>{controlStatus.textContent=event.detail.message;});
    }
    dialog.addEventListener('cancel',event=>{event.preventDefault();void this.#stop();});session.addEventListener('incoming',()=>{capture.hidden=!this.#config.allow_browser_host||!captureAvailable();captureChoices.hidden=capture.hidden;status.textContent=capture.hidden?text('Thiết bị này chưa hỗ trợ host thu màn hình.','This device cannot host screen capture.'):text('Thiết bị đã ghép nối đang xin xem. Chọn nguồn game để chấp thuận.','The paired device requested a view. Select a game capture source to approve.');});
    session.addEventListener('capture',event=>{const value=event.detail;if(pair.role==='host'&&value.active&&session.localStream&&video.srcObject!==session.localStream){video.muted=true;video.srcObject=session.localStream;void video.play().catch(()=>{});}captureStatus.textContent=!value.active?'':value.muted?text('Nguồn chia sẻ đang tạm dừng.','The shared source is temporarily paused.'):value.audio_present?value.audio_scope==='browser_reported_application'?text('Đang chia sẻ hình và âm thanh của ứng dụng đã chọn.','Sharing video and audio from the selected application.'):text('Đang chia sẻ hình và âm thanh đã cho phép.','Sharing the authorized video and audio.'):text('Đang chia sẻ hình; âm thanh chưa được truyền.','Sharing video; audio is unavailable.');});
    session.addEventListener('media_info',event=>{const value=event.detail,parts=[];if(value.codec)parts.push(value.codec.slice(6));if(value.width!==null&&value.height!==null)parts.push(`${value.width}×${value.height}`);if(value.fps!==null)parts.push(`${value.fps.toFixed(1)} FPS`);if(value.bitrate_bps!==null)parts.push(`${(value.bitrate_bps/1000000).toFixed(1)} Mb/s`);if(value.processing_ms!==null)parts.push(`${value.direction==='send'?text('mã hóa','encode'):text('giải mã','decode')} ${value.processing_ms.toFixed(1)} ms`);mediaStatus.textContent=parts.join(' · ');});
    session.addEventListener('media',event=>{video.srcObject=event.detail.stream;void video.play().catch(()=>{status.textContent=text('Nhấn Bật hình và âm thanh để tiếp tục.','Press Enable video and audio to continue.');});});session.addEventListener('state',event=>{status.textContent=event.detail.state==='connected'?text('Đã kết nối an toàn.','Secure media connection established.'):event.detail.state==='stopping'?text('Đang dừng chia sẻ…','Stopping sharing…'):event.detail.state;if(['stopping','closed'].includes(event.detail.state)){capture.hidden=true;captureChoices.hidden=true;captureStatus.textContent='';if(ownedTarget){const target=ownedTarget;ownedTarget=null;void this.#bridge.request('stream_capture_release',{capture_id:target.capture_id,session_id:target.session_id}).catch(()=>{});}}if(event.detail.state==='closed')void this.#finish(session);});session.addEventListener('session_error',event=>{this.#notify(event.detail.message,true);});
    session.addEventListener('metrics',event=>{const m=event.detail,display=v=>v===null?'—':v.toFixed(1);if(session.active)status.textContent=`RTT ${display(m.rtt_ms)} ms · jitter ${display(m.jitter_ms)} ms · loss ${display(m.packet_loss_percent)}% · decode ${display(m.decode_ms)} ms · buffer ${display(m.jitter_buffer_ms)} ms`;});const input=new RemoteInput(video,session);this.#input=input;
    if(pair.role==='client'){const touch=new RemoteTouchControls(input,this.#store,pair.id);this.#touch=touch;display.append(touch.element);dialog.append(touch.panel);void touch.load();}
    input.addEventListener('disabled',()=>{if(this.#active===session)this.#touch?.enable(false);});
    session.addEventListener('input_available',event=>{if(this.#active!==session)return;input.enable(event.detail.available);this.#touch?.enable(event.detail.available);lockPointer.hidden=!event.detail.available||pair.role!=='client';if(pair.role==='client')controlStatus.textContent=event.detail.available?text('Host đã cho phép bàn phím/chuột. Chọn màn hình game để nhập phím hoặc bật cảm ứng. Tay cầm ảo chưa khả dụng.','The host authorized keyboard/mouse. Focus the video for keys or enable touch. Virtual gamepads are unavailable.'):text('Đang xem. Điều khiển cần host cho phép riêng.','Viewing. Input requires separate host authorization.');});
    dialog.showModal();try{await transport.start();if(pair.role==='client')await session.connect();else status.textContent=text('Đang chờ thiết bị đã ghép nối yêu cầu xem.','Waiting for the paired device to request a view.');}catch(error){await this.#stop();throw error;}
  }
  async #finish(session){if(this.#active!==session)return;const native=this.#nativeInput;this.#nativeInput=null;this.#touch?.close();this.#touch=null;this.#input?.close();this.#input=null;const video=this.#dialog?.querySelector('video');if(video){video.pause();video.srcObject=null;}this.#dialog?.close();this.#dialog?.remove();this.#dialog=null;this.#active=null;this.#transport=null;await native?.close();}
  async #stop(){const session=this.#active;if(!session)return;this.#touch?.close();this.#touch=null;this.#input?.close();await session.stop();await this.#finish(session);}
  close(){clearTimeout(this.#timer);this.#abort.abort();void this.#stop().finally(()=>this.#store?.close());this.#manager?.cancelJoin();}
}
