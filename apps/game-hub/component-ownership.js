// The native broker is profile-scoped. Its transient handles remain document-
// scoped at the component boundary even when several tabs share that channel.
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const valid=id=>typeof id==='string'&&UUID.test(id)&&id!=='00000000-0000-0000-0000-000000000000';
const sessionMethods=new Set(['session_start','session_pause','session_resume','session_stop',
  'session_cancel_close','session_detach','session_disc_status','session_select_disc',
  'session_save_compatibility','save_create','save_load','save_live_export',
  'performance_native_start','performance_native_status','performance_native_cancel']);
const importGrantMethods=new Set(['library_import','library_import_start','save_import','save_repair','save_backup_restore']);
const exportGrantMethods=new Set(['save_export','save_live_export','save_export_upload_start',
  'save_export_upload_commit','diagnostics_export','performance_run_export','save_backup_export']);
const transferMethods=new Map([
  ['transfer_read','download'],['save_upload_chunk','upload'],['save_upload_commit','upload'],
  ['save_export_upload_chunk','export'],['save_export_upload_commit','export']]);
const grantCreation=new Map([['file_select_import','import'],['file_select_export','export'],['disc_select_folder','directory']]);

export const creationMethods=new Set(['stream_capture_prepare','web_session_begin','session_prepare','session_prepare_start',
  'session_prepare_status','stream_input_arm','save_export_upload_start','asset_ranges_start',
  'asset_ranges_collect','file_select_import','file_select_export','disc_select_folder',
  'asset_read_start','firmware_read_start','save_read_start','save_upload_start',
  'library_import_start','library_import_bundle_start','native_work_recover_start']);

export class DocumentOwnership {
  #documents=new Map();
  open(page){if(this.#documents.size>=64)return false;this.#documents.set(page,{grants:new Map(),transfers:new Map(),imports:new Set(),maintenance:new Set()});return true;}
  reset(){for(const state of this.#documents.values()){state.grants.clear();state.transfers.clear();state.imports.clear();state.maintenance.clear();}}

  permits(page,method,params,owned){
    const state=this.#documents.get(page);if(!state)return false;
    params=params??{};
    if(sessionMethods.has(method)&&!owned.native?.has(params.session_id))return false;
    if(method==='web_session_heartbeat'&&!owned.web?.has(params.session_id))return false;
    if(['session_prepare_status','session_prepare_cancel'].includes(method)&&!owned.preparations?.has(params.job_id))return false;
    if(['stream_input_frame','stream_input_release','stream_input_status','stream_input_revoke'].includes(method)&&(!owned.inputs?.has(params.grant_id)||owned.inputs.get(params.grant_id)!==params.session_id))return false;
    if(['asset_ranges_status','asset_ranges_collect','asset_ranges_cancel'].includes(method)&&!owned.mediaJobs?.has(params.job_id))return false;
    if(['asset_range_read','asset_ranges_keep_alive','asset_ranges_close'].includes(method)&&!owned.mediaSets?.has(params.set_id))return false;
    if(importGrantMethods.has(method)&&state.grants.get(params.grant_id)!=='import')return false;
    if(method==='library_import_bundle_start'&&state.grants.get(params.grant_id)!=='directory')return false;
    if(exportGrantMethods.has(method)&&state.grants.get(params.destination_grant_id)!=='export')return false;
    if(method==='permission_revoke'&&!state.grants.has(params.grant_id))return false;
    if(transferMethods.has(method)&&state.transfers.get(params.transfer_id)!==transferMethods.get(method))return false;
    if(method==='transfer_abort'&&!state.transfers.has(params.transfer_id))return false;
    if(['library_import_status','library_import_cancel'].includes(method)&&!state.imports.has(params.job_id))return false;
    if(['native_work_recover_status','native_work_recover_cancel'].includes(method)&&!state.maintenance.has(params.job_id))return false;
    if(grantCreation.has(method)&&state.grants.size>=64)return false;
    if(['asset_read_start','firmware_read_start','save_read_start','save_upload_start','save_export_upload_start'].includes(method)&&state.transfers.size>=64)return false;
    if(['library_import_start','library_import_bundle_start'].includes(method)&&state.imports.size>=16)return false;
    if(method==='native_work_recover_start'&&state.maintenance.size>=4)return false;
    // Only the worker disconnect/exit path owns the shared broker lifetime.
    return method!=='shutdown';
  }

  accept(page,method,params,result,deliver){
    const state=this.#documents.get(page);const cleanup=[];params=params??{};
    const remember=(collection,id,value,limit,retire)=>{
      if(!valid(id))throw new Error('invalid_native_handle');
      if(deliver&&state&&collection.size<limit){collection.set(id,value);return;}
      cleanup.push(retire(id));
      if(deliver&&state)throw Object.assign(new Error('document_resource_limit'),{cleanup});
    };
    const grant=grantCreation.get(method);
    if(grant)remember(state?.grants,result?.grant_id,grant,64,id=>({method:'permission_revoke',params:{grant_id:id}}));
    let info=null,type=null;
    if(method==='asset_read_start'){info=result;type='download';}
    if(['firmware_read_start','save_read_start'].includes(method)){info=result?.transfer;type='download';}
    if(method==='save_upload_start'){info=result;type='upload';}
    if(method==='save_export_upload_start'){info=result;type='export';}
    if(type)remember(state?.transfers,info?.transfer_id,type,64,id=>({method:'transfer_abort',params:{transfer_id:id}}));
    if(['library_import_start','library_import_bundle_start'].includes(method)){
      const id=result?.id;if(!valid(id))throw new Error('invalid_native_handle');
      if(deliver&&state&&state.imports.size<16)state.imports.add(id);
      else{cleanup.push({method:'library_import_cancel',params:{job_id:id}});if(deliver&&state)throw Object.assign(new Error('document_resource_limit'),{cleanup});}
    }
    if(method==='native_work_recover_start'){
      const id=result?.id;if(!valid(id))throw new Error('invalid_native_handle');
      if(deliver&&state&&state.maintenance.size<4)state.maintenance.add(id);
      else{cleanup.push({method:'native_work_recover_cancel',params:{job_id:id}});if(deliver&&state)throw Object.assign(new Error('document_resource_limit'),{cleanup});}
    }
    if(!state)return cleanup;
    if(importGrantMethods.has(method)||method==='library_import_bundle_start'||method==='permission_revoke')state.grants.delete(params.grant_id);
    if(exportGrantMethods.has(method)&&!['save_export_upload_start','save_export_upload_commit'].includes(method))state.grants.delete(params.destination_grant_id);
    if(method==='save_export_upload_commit')state.grants.delete(params.destination_grant_id);
    if(['save_upload_commit','save_export_upload_commit','transfer_abort'].includes(method)||method==='transfer_read'&&result?.finished===true)state.transfers.delete(params.transfer_id);
    if(method==='library_import_status'&&result?.state!=='running')state.imports.delete(params.job_id);
    if(deliver&&['native_work_recover_status','native_work_recover_cancel'].includes(method)&&['complete','cancelled','failed'].includes(result?.state))state.maintenance.delete(params.job_id);
    return cleanup;
  }

  close(page){
    const state=this.#documents.get(page);if(!state)return[];this.#documents.delete(page);
    return[
      ...Array.from(state.grants.keys(),grant_id=>({method:'permission_revoke',params:{grant_id}})),
      ...Array.from(state.transfers.keys(),transfer_id=>({method:'transfer_abort',params:{transfer_id}})),
      ...Array.from(state.imports,job_id=>({method:'library_import_cancel',params:{job_id}})),
      ...Array.from(state.maintenance,job_id=>({method:'native_work_recover_cancel',params:{job_id}}))];
  }
}
