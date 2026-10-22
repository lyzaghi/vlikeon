import{openStore}from'./idb.js';
import{WebPlatformError,requireValue,checkAbort,integer,identifier,HASH,utf8,storageError}from'./errors.js';
import{WebSaves,validateCompatibility}from'./saves.js';
import{WebLibrary}from'./library.js';
import{WebFiles}from'./files.js';
import{WebExportStreams}from'./export-stream.js';
import{WebTransfers}from'./transfers.js';
import{WebPreferences,rankWebRuntime}from'./preferences.js';
import{WebPerformance}from'./performance.js';
import{WebSessions}from'./sessions.js';
import{WebFirmware}from'./firmware.js';
import{WebSyncCredentials}from'./sync-credentials.js';
import{WebSyncAccount}from'./sync-account.js';
import{WebSyncJournal}from'./sync-journal.js';
import{WebSyncRun}from'./sync-run.js';
import{WebSyncRecovery}from'./sync-recovery.js';
import{inspectStorage}from'./storage-inventory.js';
import{WebUnusedAssets}from'./unused-assets.js';
import{firmwareRoleSizes}from'../runtime/firmware.js';
import{webCoreOptionPolicy,coreOptionRuntime}from'../runtime/core-options.js';
import{coreResourcePaths}from'../runtime/core-resources.js';
import{coreThreadPolicy}from'../runtime/thread-policy.js';
import{coreAssetPolicy}from'../runtime/core-assets.js';

export async function openWebPlatform(onDisconnected=()=>{}){
  let store;
  try{
    store=await openStore(()=>onDisconnected());
    const response=await fetch(new URL('../cores/catalog.json',import.meta.url),{credentials:'same-origin',redirect:'error'});
    requireValue(response.ok,'runtime_unavailable','The bundled runtime catalog is unavailable.');const raw=await response.text();requireValue(utf8(raw).length<=1024*1024,'resource_limit','The bundled runtime catalog exceeds its bound.');const catalog=JSON.parse(raw);
    requireValue(catalog?.schema_version===1&&Array.isArray(catalog.cores)&&catalog.cores.length<=32,'corrupt_data','The bundled runtime catalog is invalid.');const ids=new Set();
    for(const core of catalog.cores){
      requireValue(core.schema_version===1&&identifier(core.id)&&!ids.has(core.id)&&Array.isArray(core.systems)&&core.systems.length>0&&core.systems.length<=32&&typeof core.core_version==='string'&&core.core_version.length<=256&&typeof core.source_revision==='string'&&/^[a-f0-9]{40}$/.test(core.source_revision)&&HASH.test(core.wasm_sha256)&&integer(core.wasm_bytes,1,512*1024*1024),'corrupt_data','Bundled core identity is invalid.');
      coreResourcePaths(core);coreThreadPolicy(core);coreAssetPolicy(core);webCoreOptionPolicy(core);ids.add(core.id);
    }
    const platform=new WebPlatform(store,catalog.cores,onDisconnected);await platform.initialize();return platform;
  }catch(error){store?.close();throw error instanceof WebPlatformError?error:storageError(error);}
}

class WebPlatform {
  #store;#catalog;#library;#saves;#files;#exports;#transfers;#preferences;#performance;#sessions;#firmware;#syncCredentials;#syncAccount;#syncRun;#syncRecovery;#unusedAssets;#pending=new Set();#closed=false;#identity;#storageInspectActive=false;
  constructor(store,catalog){this.#store=store;this.#catalog=catalog;this.#saves=new WebSaves(store);this.#library=new WebLibrary(store);this.#files=new WebFiles();this.#exports=new WebExportStreams(this.#files);this.#transfers=new WebTransfers(this.#saves);this.#preferences=new WebPreferences(store);this.#performance=new WebPerformance(store,catalog);this.#sessions=new WebSessions(store,catalog);this.#firmware=new WebFirmware(store);}
  async initialize(){
    this.#unusedAssets=new WebUnusedAssets(this.#store);
    this.#identity={profile_id:await this.#store.config('profile_id'),device_id:await this.#store.config('device_id')};
    this.#syncCredentials=new WebSyncCredentials(this.#store,this.#identity.profile_id);
    this.#syncAccount=new WebSyncAccount(this.#store,this.#syncCredentials,this.#identity.profile_id,{assertSyncIdle:()=>this.#syncRun?.assertIdle()});
    this.#syncRecovery=new WebSyncRecovery(this.#store,this.#identity.profile_id,{available:()=>this.#syncRun?.available===true});this.#syncRun=new WebSyncRun(this.#store,this.#saves,this.#syncCredentials,this.#syncAccount,this.#identity.profile_id,{recovery:this.#syncRecovery});await this.#syncRun.initialize();this.#preferences.configureSync(this.#identity.profile_id,this.#syncRun.available);
    window.addEventListener('pagehide',event=>{if(!event.persisted)void this.close();});
  }
  async request(method,params,timeout=30000){
    requireValue(!this.#closed,'runtime_unavailable','The local browser connection is closed.');requireValue(typeof method==='string'&&/^[a-z_]{1,64}$/.test(method)&&integer(timeout,1,120000),'invalid_request','Local request is invalid.');requireValue(this.#pending.size<16,'busy','Wait for the current local operation to finish.');
    let p;try{const raw=JSON.stringify(params??{});requireValue(utf8(raw).length<=1024*1024,'resource_limit','Local request exceeds its bound.');p=JSON.parse(raw);requireValue(p&&typeof p==='object'&&!Array.isArray(p),'invalid_request','Local request parameters are invalid.');}catch(error){throw error instanceof WebPlatformError?error:new WebPlatformError('invalid_request','Local request could not be encoded.');}
    const controller=new AbortController(),timer=setTimeout(()=>controller.abort(new WebPlatformError('deadline_exceeded','The local operation exceeded its deadline.')),timeout);this.#pending.add(controller);
    try{checkAbort(controller.signal);const result=await this.#dispatch(method,p,controller.signal);if(method!=='shutdown')checkAbort(controller.signal);const encoded=JSON.stringify(result);requireValue(typeof encoded==='string'&&utf8(encoded).length<=1024*1024,'resource_limit','Local response exceeds the control-frame bound.');return JSON.parse(encoded);}
    catch(error){if(controller.signal.aborted&&controller.signal.reason instanceof WebPlatformError)throw controller.signal.reason;if(error instanceof WebPlatformError)throw error;if(error?.name==='AbortError')throw new WebPlatformError('cancelled','The operation was cancelled.');throw storageError(error);}
    finally{clearTimeout(timer);this.#pending.delete(controller);}
  }
  async #dispatch(method,p,signal){
    switch(method){
      case'hello':return{application:'OmniPlay',version:'0.1.0-source',protocol_major:1,protocol_minor:0,save_origin_protocol:1,save_repair_protocol:1,save_cloud_recovery_protocol:1,sync_settings_protocol:1,...this.#identity,platform:'web_pwa',capabilities:['local_library','file_dialog','save_history','save_export','asset_transfer','save_transfer','local_settings','local_assistance','performance_measurements','browser_storage','firmware_vault',...(this.#files.supportsDisc()?['disc_import']:[])],native_emulator:false,installed_games:false,encrypted_sync:this.#syncRun.available,desktop_updater:false};
      case'firmware_list':return this.#firmware.list({signal});
      case'sync_crypto_status':return this.#syncCredentials.status({signal});
      case'sync_transfer_status':return WebSyncJournal.status(this.#store,this.#identity.profile_id,{signal});
      case'sync_status':return this.#syncRun.status({signal});
      case'sync_opt_in_set':return this.#syncRun.configure(p,{signal});
      case'sync_run_start':return this.#syncRun.start({signal});
      case'sync_run_status':return this.#syncRun.jobStatus(p.job_id);
      case'sync_run_cancel':return this.#syncRun.cancel(p.job_id);
      case'sync_pending_export':{requireValue(Object.keys(p).length===1&&typeof p.destination_grant_id==='string','invalid_request','Choose one pending-backup export destination.');return this.#syncRun.exportPending((blob,exportSignal,guard)=>this.#files.write(p.destination_grant_id,blob,exportSignal,{guard}),{signal});}
      case'sync_pending_export_cancel':{requireValue(Object.keys(p).length===0,'invalid_request','The pending-backup cancellation has no parameters.');return this.#syncRun.cancelExport();}
      case'sync_save_recovery_request':return this.#syncRecovery.request(p.save_id,{signal});
      case'sync_save_recovery_list':return this.#syncRecovery.list(p.game_id,{signal});
      case'sync_save_recovery_cancel':return this.#syncRecovery.cancel(p.save_id,{signal});
      case'sync_action_start':return this.#syncAccount.start(p.action,{signal});
      case'sync_action_status':return this.#syncAccount.actionStatus(p.action_id);
      case'sync_action_cancel':return this.#syncAccount.cancel(p.action_id);
      case'sync_action_collect':return this.#syncAccount.collect(p.action_id);
      case'sync_action_discard':return this.#syncAccount.discard(p.action_id);
      case'firmware_import_selected':{
        try{firmwareRoleSizes(p.system,p.role);}catch{throw new WebPlatformError('unsupported_capability','This firmware role is unsupported.');}
        const opened=await this.#files.selectImport(signal),chosen=this.#files.take(opened.grant_id,'import');return this.#firmware.import(p.system,p.role,chosen.file,{signal});
      }
      case'firmware_select':return this.#firmware.select(p.system,p.role,p.content_hash,{signal});
      case'firmware_forget':return this.#firmware.forget(p.content_hash,{signal});
      case'firmware_read_start':{
        const selected=await this.#firmware.selected(p.game_id,p.role,p.expected_hash,{signal});checkAbort(signal);
        return{role:selected.role,sha256:selected.asset.content_hash,byte_length:selected.asset.byte_length,transfer:this.#transfers.open(selected.blob,selected.asset.content_hash)};
      }
      case'file_select_import':return this.#files.selectImport(signal);
      case'file_select_export':return this.#files.selectExport(p.suggested_name,signal);
      case'permission_revoke':return this.#files.revoke(p.grant_id);
      case'library_page':return this.#library.page(p);
      case'library_get':return this.#library.get(p.game_id);
      case'library_import_start':{const grant=this.#files.take(p.grant_id,'import');return this.#library.start(grant.file,p.title,p.system,p.format_hint??null);}
      case'disc_select_folder':return this.#files.selectDisc(signal);
      case'library_import_bundle_start':{const scope=this.#files.takeDisc(p.grant_id,p.entry_name);return this.#library.startBundle(scope,p.entry_name,p.title,p.system,p.encoding??'auto');}
      case'library_import_status':return this.#library.status(p.job_id);
      case'library_import_cancel':return this.#library.cancel(p.job_id);
      case'library_favorite_get':return this.#library.favorite(p.game_id);
      case'library_favorite_set':return this.#library.setFavorite(p.game_id,p.favorite,{signal});
      case'library_remove':requireValue(!this.#sessions.activeFor(p.game_id),'busy','Stop the active game before removing it.');return this.#library.remove(p.game_id,{signal});
      case'asset_read_start':{const asset=await this.#library.asset(p.game_id,p.asset_name??null);checkAbort(signal);return this.#transfers.open(asset.blob,asset.hash);}
      case'transfer_read':return this.#transfers.read(p.transfer_id,p.offset,p.limit);
      case'transfer_abort':await this.#exports.abort(p.transfer_id);return this.#transfers.abort(p.transfer_id);
      case'save_list':return this.#saves.list(p.game_id);
      case'save_page':return this.#saves.page(p.game_id,p.after??null,p.limit??50);
      case'save_heads':return this.#saves.heads(p.game_id);
      case'save_origin_get':return this.#saves.origin(p.save_id,{signal});
      case'save_purge_preview':return this.#saves.purgePreview(p.game_id);
      case'save_purge':return this.#saves.purge(p.game_id,p.expected_revision,{signal});
      case'save_cleanup':case'save_cleanup_status':return{pending_files:0,pending_bytes:0};
      case'save_promote':return this.#saves.promote(p.save_id,p.expected_head_id??null,{signal});
      case'save_read_start':{validateCompatibility(p.compatibility);const saved=await this.#saves.read(p.save_id,p.compatibility,{signal});return{metadata:saved.metadata,transfer:this.#transfers.open(saved.payload,saved.metadata.payload_hash)};}
      case'save_upload_start':await this.#library.get(p.metadata?.game_id);return this.#transfers.start(p.metadata,p.expected_parent_id??null);
      case'save_upload_chunk':return this.#transfers.chunk(p.transfer_id,p.offset,p.payload_base64);
      case'save_upload_commit':return this.#transfers.commit(p.transfer_id,{signal});
      case'save_import':{const grant=this.#files.take(p.grant_id,'import');return this.#saves.import(p.game_id,grant.file,{signal});}
      case'save_repair':{const grant=this.#files.take(p.grant_id,'import');return this.#saves.repair(p.save_id,grant.file,{signal});}
      case'save_export':return this.#files.write(p.destination_grant_id,await this.#saves.export(p.save_id,{signal}),signal);
      case'save_export_upload_start':{requireValue((await this.#library.get(p.metadata?.game_id)).content_hash===p.metadata?.game_content_hash,'content_mismatch','Export content is not in this library.');return this.#exports.start(p.metadata,p.destination_grant_id,signal);}
      case'save_export_upload_chunk':return this.#exports.chunk(p.transfer_id,p.offset,p.payload_base64,signal);
      case'save_export_upload_commit':return this.#exports.commit(p.transfer_id,p.destination_grant_id,signal);
      case'settings_get':return this.#preferences.settings();
      case'settings_set':{const settings=await this.#preferences.setSettings(p.settings,{signal});if(!settings.cloud_sync_opt_in)this.#syncRun.stop();return settings;}
      case'game_input_get':return this.#preferences.input(p.game_id);
      case'game_input_set':return this.#preferences.setInput(p.game_id,p.input,{signal});
      case'game_touch_get':return this.#preferences.touch(p.game_id);
      case'game_touch_set':return this.#preferences.setTouch(p.game_id,p.touch,{signal});
      case'game_core_options_get':return this.#preferences.coreOptions(p.game_id,p.runtime_id);
      case'game_core_options_list':return this.#preferences.coreOptionRuntimes(p.game_id);
      case'game_core_options_set':return this.#preferences.setCoreOptions(p.game_id,p.runtime_id,p.options,{signal});
      case'runtime_options_get':{coreOptionRuntime(p.runtime_id);const core=this.#catalog.find(item=>item.id===p.runtime_id);requireValue(core,'not_found','The requested runtime is not installed.');return webCoreOptionPolicy(core);}
      case'game_settings_get':return this.#preferences.get(p.game_id);
      case'game_settings_set':return this.#preferences.set(p.game_id,p.config,{signal});
      case'assistant_advise':return this.#preferences.advise(p.game_id,p.topic);
      case'assistant_accept':return this.#preferences.accept(p.proposal_id,{signal});
      case'assistant_dismiss':return this.#preferences.dismiss(p.proposal_id);
      case'runtime_rank':return rankWebRuntime(await this.#library.get(p.game_id),p.device,p.candidates,this.#catalog);
      case'runtime_list':return[];
      case'web_session_begin':return this.#sessions.begin(p.game_id,p.runtime_id,{signal});
      case'web_session_heartbeat':return this.#sessions.heartbeat(p.session_id,p.state,{signal});
      case'session_list':return this.#sessions.list();
      case'metrics_snapshot':return{};
      case'performance_run_commit':return this.#performance.commit(p.run,{signal});
      case'performance_run_list':return this.#performance.list(p.game_id);
      case'performance_run_compare':return this.#performance.compare(p.baseline_id,p.candidate_id);
      case'performance_run_export':return this.#files.write(p.destination_grant_id,await this.#performance.export(p.run_id),signal);
      case'performance_run_delete':return this.#performance.remove(p.run_id,{signal});
      case'performance_runs_clear':return this.#performance.clear({signal});
      case'diagnostics_preview':return this.#diagnostics();
      case'diagnostics_export':return this.#files.write(p.destination_grant_id,new Blob([JSON.stringify(await this.#diagnostics(),null,2)],{type:'application/json'}),signal);
      case'storage_status':return this.#storageStatus(signal);
      case'storage_inspect':{requireValue(Object.keys(p).length===0,'invalid_request','Storage inspection has no parameters.');requireValue(!this.#storageInspectActive,'busy','A stored-data inspection is still finishing.');this.#storageInspectActive=true;try{return await inspectStorage(this.#store,{signal});}finally{this.#storageInspectActive=false;}}
      case'storage_unused_preview':{requireValue(Object.keys(p).length===1&&Object.hasOwn(p,'after_hash'),'invalid_request','Choose one unused-content page.');return this.#unusedAssets.preview(p.after_hash,{signal});}
      case'storage_unused_remove':{requireValue(Object.keys(p).length===2&&Object.hasOwn(p,'after_hash')&&Object.hasOwn(p,'revision'),'invalid_request','Choose one reviewed unused-content page.');return this.#unusedAssets.remove(p.after_hash,p.revision,{signal});}
      case'storage_request_persistence':return{granted:typeof navigator.storage?.persist==='function'?await navigator.storage.persist():false};
      case'shutdown':await this.close();return{stopped:true};
      default:throw new WebPlatformError('unsupported_capability','This operation is not enabled by the current web platform policy.');
    }
  }
  async #diagnostics(){const usage=await this.#store.config('usage');return{schema_version:1,application_version:'0.1.0-source',operating_system:'browser',architecture:'unknown',exported_at_ms:Date.now(),game_count:usage?.game_count??0,save_count:usage?.save_count??0,active_sessions:this.#sessions.list().map(info=>({runtime_id:info.runtime_id,runtime_kind:info.runtime_kind,state:info.state,error_code:info.error?.code??null})),includes_roms:false,includes_save_payloads:false,includes_filesystem_paths:false,includes_browsing_history:false};}
  async #storageStatus(signal){
    let syncError=null;const[usage,firmware,estimate,persistent,sync]=await Promise.all([this.#store.config('usage'),this.#firmware.usage(),typeof navigator.storage?.estimate==='function'?navigator.storage.estimate():null,typeof navigator.storage?.persisted==='function'?navigator.storage.persisted():false,WebSyncJournal.status(this.#store,this.#identity.profile_id,{signal}).catch(error=>{syncError=error.code??'storage_failure';return null;})]);checkAbort(signal);
    return{logical_assets_bytes:usage.assets_bytes,logical_saves_bytes:usage.saves_bytes,logical_firmware_bytes:firmware.bytes,firmware_count:firmware.count,estimated_usage_bytes:Number.isFinite(estimate?.usage)?estimate.usage:null,estimated_quota_bytes:Number.isFinite(estimate?.quota)?estimate.quota:null,persistent,transaction_policy:'strict',storage_scope:'browser_origin',sync_cache:sync,sync_cache_error:syncError};
  }
  async close(){if(this.#closed)return;this.#closed=true;for(const controller of this.#pending)controller.abort();const run=this.#syncRun?.close(),account=this.#syncAccount?.close();await run;await account;this.#syncCredentials?.close();this.#library.close();this.#transfers.close();await this.#exports.close();this.#files.close();this.#preferences.close();await this.#sessions.close();this.#store.close();}
}
