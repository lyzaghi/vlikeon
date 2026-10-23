import {WebGameRuntime} from './runtime/runtime.js';

const $=id=>document.getElementById(id);
const keys=new Set(),touch=new Set();
const mapping={ArrowUp:4,KeyW:4,ArrowDown:5,KeyS:5,ArrowLeft:6,KeyA:6,ArrowRight:7,KeyD:7};
let runtime=null,manifest=null,record=null,busy=false,closing=false,pending=Promise.resolve();
function notice(message,error=false){$('notice').textContent=message;$('notice').dataset.error=String(error);}
function ui(){
  const state=runtime?.state??'created',active=['running','paused'].includes(state);
  $('stage').dataset.state=state;document.body.dataset.state=state;
  $('runtime-state').textContent=({created:'SẴN SÀNG',preparing:'ĐANG CHUẨN BỊ',ready:'SẴN SÀNG',running:'ĐANG CHƠI',paused:'TẠM DỪNG',stopped:'ĐÃ DỪNG',failed:'CẦN MỞ LẠI'})[state]??state.toUpperCase();
  $('play').disabled=busy||active;$('play').textContent=active?'Đang mở Vlike Grid':'▶  Chơi ngay';
  for(const id of ['pause','restart','save'])$(id).disabled=busy||!active;
  $('pause').textContent=state==='paused'?'Tiếp tục':'Tạm dừng';
  $('restore').disabled=busy||!active||!record;
  $('busy').hidden=!busy;
  if(record){$('save-status').textContent='Đã lưu · '+new Date(record.at).toLocaleString('vi-VN');document.body.dataset.saved='true';}
}
function enqueue(fn){
  if(closing)return pending;
  pending=pending.then(async()=>{busy=true;ui();try{await fn();}catch(error){notice(error.message||'Không thể hoàn tất thao tác.',true);}finally{busy=false;ui();}});
  return pending;
}
async function digest(bytes){return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',bytes)),value=>value.toString(16).padStart(2,'0')).join('');}
async function checkedResource(path,expected,maximum){
  const response=await fetch(path,{cache:'no-store'});if(!response.ok)throw Error('Thiếu tệp demo: '+path);
  const bytes=await response.arrayBuffer();if(!bytes.byteLength||bytes.byteLength>maximum||bytes.byteLength!==expected.bytes||await digest(bytes)!==expected.sha256)throw Error('Tệp demo không khớp bản build: '+path);
  return bytes;
}
function database(){return new Promise((resolve,reject)=>{const request=indexedDB.open('vlikeon-playable-demo-v1',1);request.onupgradeneeded=()=>request.result.createObjectStore('checkpoints');request.onsuccess=()=>resolve(request.result);request.onerror=()=>reject(Error('Không mở được vùng lưu demo trên máy.'));request.onblocked=()=>reject(Error('Đóng cửa sổ demo cũ rồi thử lại.'));});}
async function storage(value){
  const db=await database();try{return await new Promise((resolve,reject)=>{
    const transaction=db.transaction('checkpoints',value?'readwrite':'readonly'),store=transaction.objectStore('checkpoints');
    const request=value?store.put(value,'vlike-grid'):store.get('vlike-grid');let result;
    request.onsuccess=()=>{result=request.result;};transaction.oncomplete=()=>resolve(value??result??null);
    transaction.onerror=transaction.onabort=()=>reject(Error('Chưa lưu được điểm dừng. Dữ liệu đã lưu trước đó được giữ nguyên.'));
  });}finally{db.close();}
}
async function openGame(){
  $('busy-text').textContent='Đang mở core Game Boy…';
  if(runtime){await runtime.stop();runtime=null;}
  const old=$('game-canvas'),canvas=document.createElement('canvas');canvas.id='game-canvas';canvas.width=160;canvas.height=144;canvas.setAttribute('aria-label','Màn hình game Vlike Grid');old.replaceWith(canvas);
  if(!manifest){const response=await fetch('demo-manifest.json',{cache:'no-store'});if(!response.ok)throw Error('Không tìm thấy bộ dữ liệu demo.');manifest=await response.json();if(manifest.schema_version!==1||manifest.core.id!=='sameboy-web'||manifest.game.id!=='vlike-grid')throw Error('Phiên bản bộ demo không phù hợp.');}
  const [wasm,rom]=await Promise.all([checkedResource('cores/sameboy/sameboy.wasm',manifest.core,16*1024*1024),checkedResource('games/vlike-grid.gb',manifest.game,32768)]);
  runtime=new WebGameRuntime();
  runtime.addEventListener('runtime_error',event=>{notice(event.detail?.message??'Core đã dừng. Hãy mở lại game.',true);ui();});
  runtime.addEventListener('metrics',event=>{const value=event.detail;if(Number.isFinite(value?.windowDurationMs)&&value.windowDurationMs>0&&Number.isFinite(value.framesPresented))$('frame-status').textContent=`Game Boy · ${Math.round(value.framesPresented*1000/value.windowDurationMs)} khung hình/giây · ${runtime.videoGeometry.width} × ${runtime.videoGeometry.height}`;});
  await runtime.unlockAudio();
  await runtime.prepare({moduleUrl:new URL('cores/sameboy/sameboy.js',location.href).href,wasmBinary:wasm,rom,canvas,extension:'gb',options:{},volume:.4});
  $('cover').hidden=true;$('canvas-wrap').hidden=false;
  await runtime.start();
  notice('Dùng phím mũi tên hoặc WASD để nhặt quả cầu. Space để tạm dừng.');
}
async function pause(){if(runtime?.state==='running'){keys.clear();touch.clear();await runtime.pause();notice('Đã tạm dừng. Bạn có thể lưu hoặc tiếp tục.');}else if(runtime?.state==='paused'){await runtime.resume();notice('Tiếp tục cuộc chơi.');}}
async function capture({resume=true}={}){
  if(!runtime||!['running','paused'].includes(runtime.state))return;
  $('busy-text').textContent='Đang lưu điểm dừng…';const wasRunning=runtime.state==='running';if(wasRunning)await runtime.pause();
  const bytes=await runtime.save('emulator_state');if(!(bytes instanceof ArrayBuffer)||bytes.byteLength===0||bytes.byteLength>16*1024*1024)throw Error('Điểm lưu vượt giới hạn demo.');
  const value={schema:1,at:Date.now(),core:manifest.core.sha256,game:manifest.game.sha256,bytes,sha256:await digest(bytes)};
  await storage(value);record=value;if(resume&&wasRunning)await runtime.resume();notice('Đã lưu điểm dừng trên máy. Bạn có thể đóng và mở lại demo.');
}
async function restore(){
  $('busy-text').textContent='Đang khôi phục điểm dừng…';const value=await storage();
  if(!value||value.schema!==1||value.core!==manifest.core.sha256||value.game!==manifest.game.sha256||!(value.bytes instanceof ArrayBuffer)||!value.bytes.byteLength||value.bytes.byteLength>16*1024*1024||await digest(value.bytes)!==value.sha256)throw Error('Điểm lưu chưa phù hợp hoặc bị lỗi. Bản lưu cũ vẫn được giữ nguyên.');
  keys.clear();touch.clear();if(runtime.state==='running')await runtime.pause();
  await runtime.load('emulator_state',value.bytes.slice(0));await runtime.resume();record=value;notice('Đã khôi phục đúng điểm dừng.');
}
$('play').addEventListener('click',()=>enqueue(openGame));$('pause').addEventListener('click',()=>enqueue(pause));$('restart').addEventListener('click',()=>enqueue(openGame));
$('save').addEventListener('click',()=>enqueue(()=>capture()));$('restore').addEventListener('click',()=>enqueue(restore));
$('fullscreen').addEventListener('click',()=>{const result=document.fullscreenElement?document.exitFullscreen():$('stage').requestFullscreen();result.catch(()=>notice('Chưa thể chuyển toàn màn hình.',true));});
$('about').addEventListener('click',()=>enqueue(async()=>{const response=await fetch('ABOUT.txt');if(!response.ok)throw Error('Không mở được thông tin demo.');$('about-text').textContent=await response.text();$('about-dialog').showModal();}));
$('about-close').addEventListener('click',()=>$('about-dialog').close());
window.addEventListener('keydown',event=>{if(event.target.closest('button,a,input,textarea')&&!Object.hasOwn(mapping,event.code)&&event.code!=='Space')return;if(Object.hasOwn(mapping,event.code)){event.preventDefault();keys.add(event.code);}if(event.code==='Space'&&!event.repeat){event.preventDefault();enqueue(pause);}});
window.addEventListener('keyup',event=>keys.delete(event.code));
function lostFocus(){keys.clear();touch.clear();if(runtime?.state==='running')void enqueue(async()=>{if(runtime?.state==='running')await runtime.pause();});}
window.addEventListener('blur',lostFocus);document.addEventListener('visibilitychange',()=>{if(document.hidden)lostFocus();});
for(const button of document.querySelectorAll('[data-direction]')){
  button.addEventListener('pointerdown',event=>{event.preventDefault();button.setPointerCapture(event.pointerId);touch.add(Number(button.dataset.direction));});
  for(const type of ['pointerup','pointercancel','lostpointercapture'])button.addEventListener(type,()=>touch.delete(Number(button.dataset.direction)));
}
function inputs(){
  if(runtime?.state==='running'){
    const ports=Array.from({length:4},()=>Array(16).fill(0)),axes=Array.from({length:4},()=>[0,0,0,0]);
    for(const key of keys)ports[0][mapping[key]]=1;for(const id of touch)ports[0][id]=1;
    const pad=Array.from(navigator.getGamepads?.()??[]).find(value=>value?.connected&&value.mapping==='standard');
    if(pad){for(const [source,target]of [[12,4],[13,5],[14,6],[15,7],[0,8],[1,0],[9,3]])if(pad.buttons[source]?.pressed)ports[0][target]=1;if(pad.axes[0]<-.3)ports[0][6]=1;if(pad.axes[0]>.3)ports[0][7]=1;if(pad.axes[1]<-.3)ports[0][4]=1;if(pad.axes[1]>.3)ports[0][5]=1;$('gamepad-status').textContent='Đã kết nối tay cầm chuẩn Gamepad';}
    runtime.input(ports,axes);
  }requestAnimationFrame(inputs);
}
requestAnimationFrame(inputs);
window.addEventListener('vlikeon-close',()=>{
  if(closing)return;closing=true;
  pending.then(async()=>{await capture({resume:false});if(!closing){ui();return;}await runtime?.stop();if(closing)window.chrome?.webview?.postMessage('close-ready');else ui();}).catch(()=>{closing=false;window.chrome?.webview?.postMessage('close-failed');notice('Chưa lưu được trước khi đóng. Hãy lưu điểm dừng rồi thử lại.',true);});
});
window.addEventListener('vlikeon-close-cancel',()=>{closing=false;ui();});
storage().then(value=>{record=value;ui();}).catch(error=>notice(error.message,true));ui();window.chrome?.webview?.postMessage('ready');
