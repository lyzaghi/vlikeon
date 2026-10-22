import {PlatformBridge} from './bridge.js';
import {ControllerNavigation,probeDevice} from './controllers.js';
import {t,setLocale,currentLocale,exportMessage} from './i18n.js';
import {GamePlayer} from './player.js';
import {SyncSettings} from './sync-settings.js';
import {UpdateSettings} from './update-settings.js';
import {mountGameSettings} from './game-settings.js';
import {mountPerformanceHistory} from './performance-history.js';
import {NativeGamePlayer} from './native-player.js';
import {FirmwareSettings} from './firmware-settings.js';
import {LibraryImporter} from './library-import.js';
import {RemotePlayPanel} from './remote-play.js';
import {BrowserDashboard} from './browser-dashboard.js';
import {AppearanceControls} from './appearance.js';
import {SaveMaintenancePanel,removeLocalGameSaves} from './save-maintenance.js';
import {NativeWorkMaintenancePanel} from './native-work-maintenance.js';
import {mountSaveHistory} from './save-history.js';
import {DeviceInformationPanel} from './device-information.js';
import {mountInputSettings} from './input-settings.js';
import {mountTouchSettings} from './touch-settings.js';
import {mountCoreOptionSettings} from './core-option-settings.js';

const bridge=new PlatformBridge();
const $=selector=>document.querySelector(selector);
const controllers=new ControllerNavigation();
const player=new GamePlayer(bridge,controllers);
const nativePlayer=new NativeGamePlayer(bridge,controllers);
const importer=new LibraryImporter(bridge,toast,()=>loadGames(true));
const appearanceControls=new AppearanceControls($('#settings-form'));
const detailDownloads=element('div','game-export-downloads');detailDownloads.dataset.exportDownloads='';$('#detail-dialog').append(detailDownloads);
const discButton=element('button','button button-secondary',currentLocale()==='vi'?'＋ Đĩa':'＋ Disc');discButton.id='disc-import-button';discButton.addEventListener('click',()=>task(discButton,()=>importer.chooseDisc()));$('.topbar-actions').insertBefore(discButton,$('#installed-button'));
for(const instance of[player,nativePlayer])instance.addEventListener('closed',()=>task(null,()=>loadGames(true)));
player.addEventListener('warning',event=>toast(event.detail,true));
const syncContainer=document.createElement('div');
$('#view-settings').append(syncContainer);
const syncSettings=new SyncSettings(bridge,syncContainer,toast);
const updateContainer=document.createElement('div');
$('#view-settings').append(updateContainer);
const updateSettings=new UpdateSettings(bridge,updateContainer,toast);
const firmwareContainer=document.createElement('div');$('#view-settings').append(firmwareContainer);const firmwareSettings=new FirmwareSettings(bridge,firmwareContainer,toast);
const saveMaintenanceContainer=document.createElement('div');$('#view-settings').append(saveMaintenanceContainer);const saveMaintenance=new SaveMaintenancePanel(bridge,saveMaintenanceContainer,toast);
const nativeWorkContainer=document.createElement('div');$('#view-settings').append(nativeWorkContainer);const nativeWorkMaintenance=new NativeWorkMaintenancePanel(bridge,nativeWorkContainer,toast);
const deviceInformationContainer=document.createElement('div');$('#view-devices').append(deviceInformationContainer);const deviceInformation=new DeviceInformationPanel(deviceInformationContainer,bridge);
const assistantToggle=element('label');const assistantLabel=element('span','',currentLocale()==='vi'?'Gợi ý cục bộ':'Local recommendations');assistantLabel.dataset.i18n='localRecommendations';const assistantInput=element('input');assistantInput.type='checkbox';assistantInput.name='ai_enabled';assistantInput.setAttribute('role','switch');assistantToggle.append(assistantLabel,assistantInput);$('#settings-form').insertBefore(assistantToggle,$('#settings-form button[type=submit]'));
syncSettings.addEventListener('changed',()=>task(null,async()=>{state.settings=await bridge.request('settings_get');applySettings(state.settings);await loadGames(true);}));
const state={games:[],cursor:null,totalMatching:0,totalAll:0,hasMore:false,query:'',filter:'all',favoritesOnly:false,view:'library',settings:null,importGrant:null,importFormatHint:null,loading:false,pendingReload:false,connected:false};
const collectionLabel=element('label','filter-label'),collectionName=element('span','sr-only',t('collection')),collectionSelect=element('select');collectionName.dataset.i18n='collection';collectionSelect.id='collection-filter';for(const[value,key]of [['all','allGames'],['favorites','favorites']]){const option=element('option','',t(key));option.value=value;option.dataset.i18n=key;collectionSelect.append(option);}collectionLabel.append(collectionName,collectionSelect);$('.library-toolbar').append(collectionLabel);
collectionSelect.addEventListener('change',()=>{state.favoritesOnly=collectionSelect.value==='favorites';void task(null,()=>loadGames(true));});
const browserDashboard=new BrowserDashboard(bridge,$('#view-browser'),toast);
const remoteContainer=document.createElement('div');$('#view-devices').append(remoteContainer);const remotePlay=new RemotePlayPanel(remoteContainer,toast,bridge);void remotePlay.initialize();
const systemNames={nes:'NES',gb:'Game Boy',gbc:'Game Boy Color',snes:'SNES',gba:'Game Boy Advance',mega_drive:'Mega Drive',ps1:'PlayStation',n64:'Nintendo 64',nds:'Nintendo DS',psp:'PSP',pc:'PC',web:'Web'};
const systemColors={nes:'#77382f',gb:'#57673c',gbc:'#566d44',snes:'#4c4673',gba:'#454979',mega_drive:'#356078',ps1:'#56616d',n64:'#376d5d',nds:'#3b6980',psp:'#595c74',pc:'#635773',web:'#356a68'};

function element(tag,className,text){const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;}
function toast(message,isError=false){const item=element('div',`toast${isError?' error':''}`,message);$('#toast-region').append(item);setTimeout(()=>item.remove(),7000);}
function showError(error){if(error?.code==='cancelled')return;toast(error?.message||t('error'),true);}
async function task(button,action){if(button)button.disabled=true;try{return await action();}catch(error){showError(error);}finally{if(button)button.disabled=false;}}
function setConnection(connected,message){state.connected=connected;$('#connection-banner').hidden=connected||state.view==='browser';$('#connection-message').textContent=message||t('disconnected');$('#footer-status').textContent=connected?t('connected'):t('initializing');}

async function connect(){
  try{
    const platform=await bridge.request('hello');
    nativeWorkMaintenance.setAvailable(platform.native_work_maintenance_protocol===1);
    $('#installed-button').hidden=platform.installed_games===false;
    $('#disc-import-button').hidden=Array.isArray(platform.capabilities)&&!platform.capabilities.includes('disc_import');
    syncContainer.hidden=platform.encrypted_sync===false&&platform.sync_settings_protocol!==1;updateContainer.hidden=platform.desktop_updater===false;
    firmwareContainer.hidden=Array.isArray(platform.capabilities)&&!platform.capabilities.includes('firmware_vault');
    state.settings=await bridge.request('settings_get');
    applySettings(state.settings);
    setConnection(true);
    await loadGames(true);
    if(state.view==='settings')await refreshPlatformSettings();
  }catch(error){setConnection(false,error.message);}
}
async function refreshPlatformSettings(){if(!syncContainer.hidden)await syncSettings.refresh();if(!updateContainer.hidden)await updateSettings.refresh();if(!firmwareContainer.hidden)await firmwareSettings.refresh();await saveMaintenance.refresh();if(!nativeWorkContainer.hidden)await nativeWorkMaintenance.refresh();}

function applySettings(settings){
  setLocale(settings.locale);
  document.documentElement.dataset.reduceMotion=String(settings.reduce_motion);
  document.documentElement.dataset.highContrast=String(settings.high_contrast);
  document.documentElement.style.setProperty('--text-scale',String(settings.text_scale));
  appearanceControls.setSaved(settings.appearance);
  const form=$('#settings-form');
  for(const [key,value]of Object.entries(settings)){const input=form.elements.namedItem(key);if(!input)continue;if(input.type==='checkbox')input.checked=Boolean(value);else input.value=String(value);}
  $('#search-input').placeholder=currentLocale()==='vi'?'Tìm trong thư viện…':'Search your library…';
  $('#system-filter option[value=all]').textContent=currentLocale()==='vi'?'Tất cả hệ máy':'All systems';
  $('#breadcrumb-view').textContent=t(state.view);document.title='OmniPlay — '+t(state.view);
  $('#disc-import-button').textContent=currentLocale()==='vi'?'＋ Đĩa':'＋ Disc';
  renderGames();
  renderControllers(controllers.connected());
}

async function loadGames(reset=false){
  if(state.loading){if(reset)state.pendingReload=true;return;}
  state.loading=true;
  try{
    if(reset){state.cursor=null;state.games=[];}
    const query=state.query,system=state.filter,favoritesOnly=state.favoritesOnly;
    const page=await bridge.request('library_page',{query:query||null,system:system==='all'?null:system,after:state.cursor,limit:100,favorites_only:favoritesOnly});
    if(query!==state.query||system!==state.filter||favoritesOnly!==state.favoritesOnly){state.pendingReload=true;return;}
    const known=new Set(state.games.map(game=>game.id));state.games.push(...page.games.filter(game=>!known.has(game.id)));
    state.cursor=page.next_cursor;state.hasMore=Boolean(page.next_cursor);state.totalMatching=page.total_matching;state.totalAll=page.total_all;
    renderGames();
  }finally{state.loading=false;if(state.pendingReload){state.pendingReload=false;await loadGames(true);}}
}

function cover(game){
  const image=element('div','game-cover');
  image.style.setProperty('--cover-color',systemColors[game.system]||'#314753');
  image.append(element('span','cover-system',systemNames[game.system]||game.system),element('span','cover-initial',game.title.slice(0,2).toUpperCase()),element('span','cover-title',game.title));
  image.setAttribute('aria-hidden','true');
  return image;
}

function renderGames(){
  const grid=$('#game-grid');grid.replaceChildren();
  const games=state.games.filter(game=>state.filter==='all'||game.system===state.filter);
  for(const game of games){
    const button=element('button','game-card');button.type='button';button.setAttribute('aria-label',`${t('selectGame')}: ${game.title}`);
    button.append(cover(game),element('h3','',(game.favorite?'★ ':'')+game.title),element('p','',systemNames[game.system]||game.system),element('span','compatibility',t('compatibilityUnknown')));
    button.addEventListener('click',()=>task(button,()=>openGame(game)));grid.append(button);
  }
  $('#empty-library').hidden=state.totalAll>0||state.query.length>0;
  $('#result-count').textContent=games.length?`${games.length} / ${state.totalMatching} ${t('count')}`:(state.query||state.filter!=='all'||state.favoritesOnly)?t('noResults'):'';
  $('#load-more-button').hidden=!state.hasMore;
  $('#library-count').textContent=String(state.totalAll);
  const list=$('#save-game-list');list.replaceChildren();
  for(const game of games){const button=element('button','list-row');const text=element('span');text.append(element('strong','',game.title),element('small','',systemNames[game.system]));button.append(text,element('span','','→'));button.addEventListener('click',()=>task(button,()=>openGame(game,true)));list.append(button);}
  if(!games.length)list.append(element('p','muted',t('noResults')));
}

async function openGame(game,showSaves=false){
  game=await bridge.request('library_get',{game_id:game.id});
  const dialog=$('#detail-dialog'),content=$('#detail-content');content.replaceChildren();
  const heading=element('div','dialog-heading');heading.append(element('h2','',game.title));
  const close=element('button','icon-button','×');close.setAttribute('aria-label',t('close'));close.addEventListener('click',()=>dialog.close());heading.append(close);content.append(heading);
  const layout=element('div','detail-layout');layout.append(cover(game));
  const description=element('div','detail-description');description.append(element('p','eyebrow',t('localGame')),element('p','muted',`${systemNames[game.system]} · ${formatBytes(game.byte_length)}`),element('p','compatibility',t('compatibilityUnknown')));
  const play=element('button','button button-primary',`▷ ${t('play')}`);play.addEventListener('click',()=>task(play,async()=>{
    dialog.close();
    if(game.system==='pc'){await bridge.request('installed_game_launch',{game_id:game.id});toast(currentLocale()==='vi'?'Đã mở game trên máy.':'Installed game started.');}
    else await player.open(game);
  }));description.append(play);layout.append(description);content.append(layout);
  const actions=element('div','detail-actions');
  const purgeButton=element('button','button button-quiet',currentLocale()==='vi'?'Xoá lịch sử lưu trên máy này':'Remove local save history');purgeButton.type='button';purgeButton.addEventListener('click',()=>task(purgeButton,async()=>{const result=await removeLocalGameSaves(bridge,game,toast);if(!result)return;await loadGames(true);await openGame(game,true);if(result.cleanup.pending_files)void saveMaintenance.run();else await saveMaintenance.refresh();}));actions.append(purgeButton);
  let favorite=false;const favoriteButton=element('button','button button-secondary',t('favorites'));favoriteButton.type='button';favoriteButton.disabled=true;favoriteButton.setAttribute('aria-pressed','false');
  const renderFavorite=()=>{favoriteButton.textContent=t(favorite?'removeFavorite':'addFavorite');favoriteButton.setAttribute('aria-pressed',String(favorite));};
  void bridge.request('library_favorite_get',{game_id:game.id}).then(value=>{favorite=value.favorite;renderFavorite();favoriteButton.disabled=false;}).catch(showError);
  favoriteButton.addEventListener('click',()=>task(favoriteButton,async()=>{const value=await bridge.request('library_favorite_set',{game_id:game.id,favorite:!favorite});favorite=value.favorite;renderFavorite();await loadGames(true);}));actions.append(favoriteButton);
  const importSave=element('button','button button-secondary',currentLocale()==='vi'?'Nhập bản lưu':'Import saved progress');
  importSave.addEventListener('click',()=>task(importSave,async()=>{
    const grant=await bridge.request('file_select_import',undefined,120000);
    const saved=await bridge.request('save_import',{grant_id:grant.grant_id,game_id:game.id},120000);
    toast(saved.conflict?(currentLocale()==='vi'?'Đã giữ bản nhập thành nhánh riêng.':'Imported progress was preserved as a separate branch.'):t('imported'));
    await openGame(game,true);
  }));actions.append(importSave);
  const remove=element('button','button button-quiet',t('remove'));
  remove.addEventListener('click',()=>task(remove,async()=>{if(!confirm(t('removeConfirm')))return;await bridge.request('library_remove',{game_id:game.id});dialog.close();await loadGames(true);toast(t('removed'));}));actions.append(remove);content.append(actions);
  if(game.system!=='pc'){
    void bridge.request('runtime_list').then(runtimes=>{for(const runtime of runtimes.filter(item=>item.systems.includes(game.system))){const button=element('button','button button-secondary',`${runtime.id} · Native`);button.addEventListener('click',()=>task(button,async()=>{dialog.close();await nativePlayer.open(game,runtime);}));actions.append(button);}}).catch(showError);
    const preferences=element('section','panel game-preferences'),performance=element('section','panel game-preferences'),input=element('section','panel game-preferences'),touch=element('section','panel game-preferences'),coreOptions=element('section','panel game-preferences');content.append(preferences,coreOptions,input,touch,performance);void mountGameSettings(preferences,bridge,game,toast);void mountCoreOptionSettings(coreOptions,bridge,game,toast);void mountInputSettings(input,bridge,game,toast);void mountTouchSettings(touch,bridge,game,toast);void mountPerformanceHistory(performance,bridge,game,toast);
  }
  const saves=element('section','save-list');saves.append(element('h3','',t('saves')));content.append(saves);if(!dialog.open)dialog.showModal();
  if(showSaves)saves.scrollIntoView({block:'nearest'});
  await mountSaveHistory(saves,bridge,game,toast);
}

function formatBytes(bytes){if(bytes<1024)return`${bytes} B`;if(bytes<1024*1024)return`${(bytes/1024).toFixed(1)} KB`;return`${(bytes/(1024*1024)).toFixed(1)} MB`;}
async function importGame(){
  const grant=await bridge.request('file_select_import',undefined,120000);
  state.importGrant=grant.grant_id;
  state.importFormatHint=grant.format_hint||null;
  $('#import-form').elements.title.value=grant.suggested_title;
  $('#import-dialog').showModal();
}

function changeView(view){
  if(!['library','saves','devices','settings','browser'].includes(view))return;
  if(state.view==='settings'&&view!=='settings')appearanceControls.revertPreview();
  state.view=view;
  document.body.dataset.view=view;
  browserDashboard.setActive(view==='browser');
  nativeWorkMaintenance.setActive(view==='settings');
  $('#connection-banner').hidden=state.connected||view==='browser';
  document.querySelectorAll('.view').forEach(node=>node.hidden=node.id!==`view-${view}`);
  document.querySelectorAll('[data-view]').forEach(node=>{const selected=node.dataset.view===view;node.classList.toggle('selected',selected);selected?node.setAttribute('aria-current','page'):node.removeAttribute('aria-current');});
  $('#breadcrumb-view').textContent=t(view);document.title='OmniPlay — '+t(view);
  history.replaceState(null,'',`#${view}`);
  if(view==='devices')void renderCapabilities();
  if(view==='settings'&&state.connected)void refreshPlatformSettings();
}

function renderControllers(pads){
  $('#controller-status').textContent=pads.length?`${pads.length} ${t('controllerConnected')}`:t('noController');
  const list=$('#controller-list');list.replaceChildren();
  if(!pads.length){list.append(element('p','muted small',t('noController')));return;}
  for(const pad of pads){const row=element('div','controller-card'),text=element('div');text.append(element('strong','',pad.id),element('small','',`${pad.buttons} buttons · ${pad.axes} axes · ${pad.mapping||'custom'}`));row.append(element('span','controller-symbol','⌁'),text);list.append(row);}
}

async function renderCapabilities(){
  void deviceInformation.refresh();
  const probe=await probeDevice(),list=$('#capability-list');list.replaceChildren();
  const labels={webassembly:'WebAssembly',threads:'WASM threads / isolation',webgpu:'WebGPU',webgl2:'WebGL 2',hardwareConcurrency:'Logical processors',gamepad:'Gamepad API'};
  for(const[key,value]of Object.entries(probe)){const row=element('div');row.append(element('dt','',labels[key]),element('dd','',typeof value==='boolean'?t(value?'available':'unavailable'):String(value??'—')));list.append(row);}
}

async function diagnostics(){
  const preview=await bridge.request('diagnostics_preview');
  const dialog=$('#detail-dialog'),content=$('#detail-content');content.replaceChildren(element('h2','',t('diagnostics')),element('p','muted small',t('diagnosticsDescription')));
  content.append(element('pre','diagnostic-json',JSON.stringify(preview,null,2)));
  const actions=element('div','detail-actions'),exportButton=element('button','button button-primary',t('exportDiagnostics')),close=element('button','button button-secondary',t('close'));
  close.addEventListener('click',()=>dialog.close());
  exportButton.addEventListener('click',()=>task(exportButton,async()=>{const grant=await bridge.request('file_select_export',{suggested_name:'omniplay-diagnostics.json'},120000);const result=await bridge.request('diagnostics_export',{destination_grant_id:grant.grant_id},120000);toast(exportMessage(result));dialog.close();}));
  actions.append(exportButton,close);content.append(actions);dialog.showModal();
}

$('#import-button').addEventListener('click',event=>task(event.currentTarget,importGame));
$('#installed-button').addEventListener('click',()=>$('#installed-dialog').showModal());
$('#installed-form').addEventListener('submit',event=>{event.preventDefault();const title=event.currentTarget.elements.title.value.trim();task(event.submitter,async()=>{await bridge.request('installed_game_select',{title},120000);$('#installed-dialog').close();await loadGames(true);toast(t('imported'));});});
$('#empty-import-button').addEventListener('click',event=>task(event.currentTarget,importGame));
$('#import-form').addEventListener('submit',event=>{event.preventDefault();task(event.submitter,async()=>{if(!state.importGrant)return;const form=event.currentTarget;await importer.startFile(state.importGrant,form.elements.title.value.trim(),form.elements.system.value,state.importFormatHint);state.importGrant=null;$('#import-dialog').close();});});
$('#import-dialog').addEventListener('close',()=>{if(state.importGrant){const grant_id=state.importGrant;state.importGrant=null;bridge.request('permission_revoke',{grant_id}).catch(()=>{});}});
document.querySelectorAll('[data-close-dialog]').forEach(button=>button.addEventListener('click',()=>button.closest('dialog').close()));
document.querySelectorAll('[data-view]').forEach(button=>button.addEventListener('click',()=>changeView(button.dataset.view)));
let searchTimer;$('#search-input').addEventListener('input',event=>{clearTimeout(searchTimer);const query=event.target.value;searchTimer=setTimeout(()=>{state.query=query;task(null,()=>loadGames(true));},220);});
$('#system-filter').addEventListener('change',event=>{state.filter=event.target.value;void task(null,()=>loadGames(true));});
$('#load-more-button').addEventListener('click',event=>task(event.currentTarget,()=>loadGames(false)));
$('#reconnect-button').addEventListener('click',event=>task(event.currentTarget,connect));
$('#diagnostics-button').addEventListener('click',event=>task(event.currentTarget,diagnostics));
$('#settings-form').addEventListener('submit',event=>{event.preventDefault();const form=event.currentTarget;task(event.submitter,async()=>{const latest=await bridge.request('settings_get');const updated={...latest,locale:form.elements.locale.value,text_scale:Number(form.elements.text_scale.value),reduce_motion:form.elements.reduce_motion.checked,high_contrast:form.elements.high_contrast.checked,ai_enabled:form.elements.ai_enabled.checked,appearance:appearanceControls.value()};state.settings=await bridge.request('settings_set',{settings:updated});applySettings(state.settings);await refreshPlatformSettings();toast(t('saved'));});});
bridge.addEventListener('host_disconnected',()=>setConnection(false));
controllers.addEventListener('controllers_changed',event=>renderControllers(event.detail));
controllers.start();
changeView(location.hash.slice(1)||'library');
setLocale('vi');
void browserDashboard.initialize().then(available=>{$('#browser-nav').hidden=!available;if(!available&&state.view==='browser')changeView('library');});
await connect();
