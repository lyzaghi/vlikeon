import {currentLocale} from './i18n.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const UUID=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const node=(tag,className)=>{const item=document.createElement(tag);if(className)item.className=className;return item;};
const integer=(value,max)=>Number.isSafeInteger(value)&&value>=0&&value<=max;

function storageStatus(value){
  if(!value||value.schema_version!==1||!integer(value.works,64)||
    !integer(value.reserved_works,64)||!integer(value.staged_works,64)||!integer(value.retiring_works,64)||
    value.reserved_works+value.staged_works+value.retiring_works!==value.works||
    !integer(value.reserved_bytes,32*1024**3)||!integer(value.reserved_files,4096)||
    value.maximum_works!==64||value.maximum_bytes!==32*1024**3||value.maximum_files!==4096||
    value.physical_bytes!==null||typeof value.recovery_running!=='boolean')throw new Error('invalid_native_storage_status');
  return value;
}

function jobStatus(value,expected=null){
  const summary=value?.summary;
  if(!value||typeof value.id!=='string'||!UUID.test(value.id)||value.id==='00000000-0000-0000-0000-000000000000'||
    expected!==null&&value.id!==expected||!['running','stopping','complete','cancelled','failed'].includes(value.state)||
    !integer(value.started_at_ms,Number.MAX_SAFE_INTEGER)||
    value.finished_at_ms!==null&&!integer(value.finished_at_ms,Number.MAX_SAFE_INTEGER)||
    !summary||!integer(summary.attempted,32)||!integer(summary.recovered,32)||!integer(summary.retained,32)||
    summary.recovered+summary.retained!==summary.attempted||!integer(summary.unknown_entries,512)||
    typeof summary.cancelled!=='boolean'||typeof summary.more_available!=='boolean')throw new Error('invalid_native_recovery_status');
  // Older brokers omit this additive diagnostic. Never infer deletion rights
  // or ownership from it, and reject inconsistent/unbounded newer responses.
  const retention=summary.retention;
  if(retention!==undefined){
    const keys=['locked','legacy_ownership','incomplete_ownership','other_platform','unconfirmed'];
    if(!retention||typeof retention!=='object'||Array.isArray(retention)||Object.keys(retention).length!==keys.length||
      keys.some(key=>!Object.hasOwn(retention,key)||!integer(retention[key],32))||
      keys.reduce((total,key)=>total+retention[key],0)!==summary.retained)throw new Error('invalid_native_retention_status');
  }
  return value;
}

export class NativeWorkMaintenancePanel {
  #bridge;#root;#notify;#available=false;#active=false;#generation=0;#status=null;#job=null;
  #starting=false;#polling=false;#refreshing=false;#cancelling=false;#stopRequested=false;#timer=null;#failure=false;
  #cancelFailed=false;
  #heading;#description;#storage;#progress;#start;#stop;#refresh;

  constructor(bridge,root,notify){
    this.#bridge=bridge;this.#root=root;this.#notify=notify;
    const section=node('section','panel native-work-maintenance');
    this.#heading=node('h2');this.#description=node('p','muted small');
    this.#storage=node('p','muted small');this.#progress=node('p','muted small');
    this.#progress.setAttribute('role','status');this.#progress.setAttribute('aria-live','polite');
    const actions=node('div','detail-actions');
    this.#start=node('button','button button-secondary');this.#stop=node('button','button button-quiet');this.#refresh=node('button','button button-quiet');
    for(const button of[this.#start,this.#stop,this.#refresh])button.type='button';
    this.#start.addEventListener('click',()=>void this.#begin());
    this.#stop.addEventListener('click',()=>{this.#stopRequested=true;this.#cancelFailed=false;this.#render();void this.#cancel();});
    this.#refresh.addEventListener('click',()=>void this.refresh());
    actions.append(this.#start,this.#stop,this.#refresh);section.append(this.#heading,this.#description,this.#storage,this.#progress,actions);root.replaceChildren(section);
    document.addEventListener('omniplay_locale_changed',()=>this.#render());
    document.addEventListener('visibilitychange',()=>{if(document.hidden)this.#clearTimer();else this.#schedule();});
    window.addEventListener('pagehide',()=>{this.#stopRequested=true;this.#clearTimer();void this.#cancel();});
    window.addEventListener('pageshow',()=>this.#schedule());
    bridge.addEventListener('host_disconnected',()=>this.disconnect());this.#render();
  }

  setAvailable(value){
    if(value!==true){this.disconnect();return;}
    this.#available=true;this.#root.hidden=false;this.#render();
  }
  setActive(value){this.#active=value===true;if(!this.#active)this.#clearTimer();else this.#schedule();}
  disconnect(){
    this.#generation++;this.#available=false;this.#root.hidden=true;this.#clearTimer();
    this.#status=null;this.#job=null;this.#starting=false;this.#polling=false;this.#refreshing=false;this.#cancelling=false;this.#stopRequested=false;this.#cancelFailed=false;this.#failure=false;this.#render();
  }
  #running(){return this.#job&&['running','stopping'].includes(this.#job.state);}
  #clearTimer(){clearTimeout(this.#timer);this.#timer=null;}
  #schedule(){
    this.#clearTimer();
    if(!this.#available||!this.#active||document.hidden)return;
    if(this.#running()&&!this.#polling)this.#timer=setTimeout(()=>{this.#timer=null;void this.#poll();},750);
    else if(this.#status?.recovery_running&&!this.#refreshing)this.#timer=setTimeout(()=>{this.#timer=null;void this.refresh();},2000);
  }

  async refresh(){
    if(!this.#available||this.#refreshing)return;
    const generation=this.#generation;this.#refreshing=true;this.#render();
    try{const result=await this.#bridge.request('native_work_status',undefined,15000);if(generation===this.#generation){this.#status=storageStatus(result);this.#failure=false;}}
    catch{if(generation===this.#generation){this.#failure=true;this.#notify(text('Chưa đọc được trạng thái dữ liệu tạm. Bạn có thể thử làm mới.','Temporary-session status is unavailable. You can refresh and retry.'),true);}}
    finally{if(generation===this.#generation){this.#refreshing=false;this.#render();if(!this.#failure)this.#schedule();}}
  }

  async #begin(){
    if(!this.#available||this.#starting||this.#running()||this.#refreshing||this.#status?.recovery_running)return;
    const generation=this.#generation;this.#starting=true;this.#stopRequested=false;this.#cancelFailed=false;this.#failure=false;this.#job=null;this.#render();
    try{
      const result=await this.#bridge.request('native_work_recover_start',undefined,15000);
      if(generation!==this.#generation)return;
      this.#job=jobStatus(result);
      if(this.#stopRequested)await this.#cancel();else this.#schedule();
    }catch(error){if(generation===this.#generation){this.#failure=true;this.#notify(error?.code==='busy'?text('Một lượt kiểm tra khác vẫn đang kết thúc. Hãy thử lại sau.','Another recovery operation is still finishing. Try again later.'):text('Chưa bắt đầu được lượt kiểm tra. Hãy làm mới trước khi thử lại.','Recovery could not start. Refresh before retrying.'),true);}}
    finally{if(generation===this.#generation){this.#starting=false;this.#render();this.#schedule();}}
  }

  async #cancel(){
    if(!this.#available||!this.#running()||this.#cancelling)return;
    const generation=this.#generation,id=this.#job.id;this.#cancelling=true;this.#cancelFailed=false;this.#clearTimer();this.#render();
    try{const result=await this.#bridge.request('native_work_recover_cancel',{job_id:id},15000);if(generation===this.#generation&&this.#job?.id===id){this.#job=jobStatus(result,id);if(!this.#running())await this.refresh();}}
    catch{if(generation===this.#generation){this.#failure=true;this.#cancelFailed=true;this.#notify(text('Chưa xác nhận được yêu cầu dừng. Lượt hiện tại có thể vẫn đang kết thúc.','The stop request could not be confirmed. The current operation may still be finishing.'),true);}}
    finally{if(generation===this.#generation){this.#cancelling=false;this.#render();this.#schedule();}}
  }

  async #poll(){
    if(!this.#available||!this.#running()||this.#polling||!this.#active||document.hidden)return;
    const generation=this.#generation,id=this.#job.id;this.#polling=true;
    try{
      const result=await this.#bridge.request('native_work_recover_status',{job_id:id},15000);
      if(generation!==this.#generation||this.#job?.id!==id)return;
      this.#job=jobStatus(result,id);this.#failure=false;this.#render();
      if(!this.#running())await this.refresh();
    }catch{if(generation===this.#generation){this.#failure=true;this.#clearTimer();this.#notify(text('Chưa đọc được kết quả lượt kiểm tra. Dữ liệu chưa xác nhận vẫn được giữ lại.','The recovery result could not be read. Unconfirmed data remains retained.'),true);}}
    finally{if(generation===this.#generation){this.#polling=false;this.#render();if(!this.#failure)this.#schedule();}}
  }

  #render(){
    this.#root.hidden=!this.#available;
    this.#heading.textContent=text('Dữ liệu tạm của phiên chơi','Temporary game-session data');
    this.#description.textContent=text('Kiểm tra và thu hồi dữ liệu tạm của các phiên đã kết thúc. Dữ liệu chưa xác minh được được giữ lại để khôi phục.','Inspect and recover temporary data from ended sessions. Unconfirmed data stays retained for recovery.');
    this.#storage.textContent=this.#status?text(`${this.#status.works} phiên đang giữ · ${(this.#status.reserved_bytes/1024**2).toFixed(1)} MiB dung lượng dự trù. Đây chưa phải số đo dung lượng đĩa thực tế.`,`${this.#status.works} retained sessions · ${(this.#status.reserved_bytes/1024**2).toFixed(1)} MiB reserved. This is not a measurement of actual disk allocation.`):text('Chưa đọc trạng thái dữ liệu tạm.','Temporary-session status has not been read.');
    const summary=this.#job?.summary;
    let message=this.#starting?text('Đang bắt đầu lượt kiểm tra…','Starting recovery…'):summary?text(`Đã kiểm tra ${summary.attempted} mục · thu hồi ${summary.recovered} · giữ lại ${summary.retained}.`,`Checked ${summary.attempted} entries · recovered ${summary.recovered} · retained ${summary.retained}.`):text('Chưa có lượt kiểm tra trong tab này.','No recovery operation has run in this tab.');
    if(this.#running())message+=' '+text('Đang xử lý.','Working.');
    else if(this.#status?.recovery_running)message+=' '+text('Một lượt kiểm tra khác đang kết thúc.','Another recovery operation is finishing.');
    if(this.#stopRequested&&(this.#starting||this.#running()))message+=' '+text('Đã yêu cầu dừng sau thao tác hiện tại.','Stop requested after the current operation.');
    if(this.#job?.state==='cancelled')message+=' '+text('Đã dừng lượt kiểm tra.','Recovery stopped.');
    if(this.#job?.state==='failed'||this.#failure)message+=' '+text('Chưa hoàn tất; hãy làm mới để kiểm tra lại.','Not finished; refresh to inspect again.');
    if(summary?.unknown_entries)message+=' '+text(`${summary.unknown_entries} mục chưa nhận diện được vẫn được giữ lại.`,`${summary.unknown_entries} unrecognized entries remain retained.`);
    const retention=summary?.retention;
    if(retention?.locked)message+=' '+text(`${retention.locked} mục đang bị khóa bởi thao tác khác; đóng phiên chơi hoặc chờ thao tác đó kết thúc rồi thử lại.`,`${retention.locked} entries are locked by another operation; close the game or wait for that operation to finish before retrying.`);
    if(retention?.legacy_ownership||retention?.incomplete_ownership){
      const legacy=retention.legacy_ownership,incomplete=retention.incomplete_ownership;
      message+=' '+text(`${legacy} mục cũ và ${incomplete} mục thiếu xác nhận sở hữu chưa đủ bằng chứng để thu hồi. Giữ bản sao lưu hồ sơ khi cần hỗ trợ phục hồi; chạy lại không tự bổ sung bằng chứng còn thiếu.`,`${legacy} legacy entries and ${incomplete} entries with incomplete ownership lack the evidence required for recovery. Keep a profile backup when seeking recovery support; rerunning does not supply the missing evidence.`);
    }
    if(retention?.other_platform)message+=' '+text(`${retention.other_platform} mục cần công cụ phục hồi của hệ điều hành tương ứng.`,`${retention.other_platform} entries require recovery on their corresponding operating system.`);
    if(retention?.unconfirmed)message+=' '+text(`${retention.unconfirmed} mục chưa thể xác minh; dữ liệu vẫn được giữ lại.`,`${retention.unconfirmed} entries could not be confirmed; their data remains retained.`);
    if(summary?.retained||summary?.unknown_entries)message+=' '+text('Bạn có thể xem và xuất Báo cáo chẩn đoán trong Cài đặt để hỗ trợ kiểm tra phục hồi. Báo cáo không phải bản sao lưu dữ liệu.', 'You can preview and export Diagnostics in Settings for recovery review. The report is not a data backup.');
    if(summary?.more_available&&!this.#running())message+=' '+text('Còn mục chưa kiểm tra; bạn có thể chạy tiếp một lượt.','More entries remain; you can run another pass.');
    if(this.#progress.textContent!==message)this.#progress.textContent=message;
    this.#start.textContent=summary?.more_available?text('Tiếp tục kiểm tra','Continue inspection'):text('Kiểm tra và thu hồi','Inspect and recover');
    this.#start.disabled=!this.#available||this.#starting||Boolean(this.#running())||this.#refreshing||!this.#status||this.#status.recovery_running;
    this.#stop.textContent=this.#cancelFailed?text('Thử yêu cầu dừng lại','Retry stop request'):text('Dừng sau thao tác hiện tại','Stop after current operation');
    this.#stop.disabled=!this.#available||!this.#starting&&!this.#running()||this.#cancelling||this.#stopRequested&&!this.#cancelFailed;
    this.#refresh.textContent=text('Làm mới','Refresh');this.#refresh.disabled=!this.#available||this.#refreshing;
  }
}
