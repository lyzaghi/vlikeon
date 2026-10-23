import{t,currentLocale,exportMessage}from'./i18n.js';
import{checkedSaveOrigin}from'./save-origin.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,className,value)=>{const item=document.createElement(tag);if(className)item.className=className;if(value!==undefined)item.textContent=value;return item;};
export async function mountSaveHistory(container,bridge,game,notify){
  const title=node('h3','',t('saves')),status=node('p','muted small'),rows=node('div','save-history-rows'),controls=node('div','detail-actions');status.setAttribute('role','status');
  const previous=node('button','button button-secondary',text('Trang trước','Previous page')),next=node('button','button button-secondary',text('Trang sau','Next page')),refresh=node('button','button button-quiet',text('Làm mới lịch sử','Refresh history')),backup=node('button','button button-secondary',text('Sao lưu trang này','Back up this page')),restore=node('button','button button-secondary',text('Khôi phục gói sao lưu','Restore backup archive'));
  backup.hidden=restore.hidden=true;for(const button of[previous,next,refresh,backup,restore])button.type='button';controls.append(previous,next,refresh,backup,restore);container.replaceChildren(title,status,rows,controls);
  const cursors=[null];let pageIndex=0,following=null,busy=false,revision=0,pageIds=[];
  const support=bridge.request('hello').then(value=>({orphan:value.save_orphan_protocol===1,backup:value.save_backup_protocol===1,origins:value.save_origin_protocol===1,repair:value.save_repair_protocol===1,cloud:value.save_cloud_recovery_protocol===1&&value.encrypted_sync===true})).catch(()=>({orphan:false,backup:false,origins:false,repair:false,cloud:false}));
  function lock(value){busy=value;for(const button of container.querySelectorAll('button'))button.disabled=value;previous.disabled=value||pageIndex===0;next.disabled=value||!following;backup.disabled=value||!pageIds.length;}
  async function task(action){if(busy)return;lock(true);try{return await action();}catch(error){if(error?.code!=='cancelled')notify(error.message||t('error'),true);}finally{if(container.isConnected)lock(false);}}
  async function load(reset=false){
    const oldCursors=[...cursors],oldIndex=pageIndex,oldStatus=status.textContent;if(reset){cursors.splice(0,cursors.length,null);pageIndex=0;}const ticket=++revision;status.textContent=text('Đang lấy lịch sử…','Loading history…');
    let page,capabilities;try{[page,capabilities]=await Promise.all([bridge.request('save_page',{game_id:game.id,after:cursors[pageIndex],limit:50}),support]);}catch(error){if(reset){cursors.splice(0,cursors.length,...oldCursors);pageIndex=oldIndex;}status.textContent=oldStatus||text('Chưa lấy được lịch sử lưu. Bạn có thể làm mới để thử lại.','Save history is unavailable. Refresh to try again.');throw error;}if(!container.isConnected||ticket!==revision)return;
    let cloudEnabled=false,cloudRecords=new Map(),cloudError=false;
    if(capabilities.cloud){try{
      const[state,records]=await Promise.all([bridge.request('sync_status'),bridge.request('sync_save_recovery_list',{game_id:game.id})]);
      cloudEnabled=state.connected===true&&state.enabled===true&&state.account?.registered===true;
      cloudRecords=recoveryRecords(records);
    }catch{cloudError=true;}}
    if(!container.isConnected||ticket!==revision)return;
    following=page.next_cursor;pageIds=page.entries.map(entry=>entry.metadata.id);backup.hidden=restore.hidden=!capabilities.backup;rows.replaceChildren();status.textContent=text(`Trang ${pageIndex+1} · ${page.entries.length} bản lưu · Tổng ${page.total}`,`Page ${pageIndex+1} · ${page.entries.length} saves · ${page.total} total`);
    if(cloudError)status.append(node('small','muted',text('Chưa lấy được trạng thái khôi phục từ đồng bộ. Làm mới để thử lại.','Cloud recovery status is unavailable. Refresh to try again.')));
    if(!page.entries.length)rows.append(node('p','muted small',page.total?t('noResults'):t('noSaves')));
    for(const entry of page.entries){
      const save=entry.metadata,row=node('div','save-item'),description=node('div'),date=new Date(save.created_at_ms);
      description.append(node('p','',`${t('generation')} ${save.generation} · ${save.kind==='in_game'?text('Trong game','In-game'):text('Trạng thái','Snapshot')} · ${text('Ô','Slot')} ${save.slot+1}`),node('small','',`${Number.isFinite(date.getTime())?date.toLocaleString(currentLocale()):text('Thời điểm chưa xác định','Unknown time')} · ${save.core_id} ${save.core_version}`));
      if(save.label)description.append(node('small','',save.label));if(entry.current_head_id===save.id)description.append(node('small','',text('Bản đang sử dụng trong ô này','Current progress in this slot')));else if(save.conflict)description.append(node('small','',text('Nhánh được giữ riêng','Preserved branch')));
      const cloud=cloudRecords.get(save.id);
      if(cloud)description.append(node('small','',recoveryLabel(cloud)));
      const actions=node('div','detail-actions'),exportButton=node('button','button button-secondary',t('export'));exportButton.type='button';exportButton.addEventListener('click',()=>void task(async()=>{const grant=await bridge.request('file_select_export',{suggested_name:`omniplay-save-${save.id}.opsave`},120000);const result=await bridge.request('save_export',{save_id:save.id,destination_grant_id:grant.grant_id},120000);notify(exportMessage(result));}));actions.append(exportButton);
      if(capabilities.origins){const origin=node('button','button button-secondary',text('Nguồn gốc','Origin'));origin.type='button';origin.addEventListener('click',()=>void task(async()=>{const owner=revision,value=checkedSaveOrigin(await bridge.request('save_origin_get',{save_id:save.id}),save);if(owner!==revision||!container.isConnected||!row.isConnected)return;showOrigin(value,save);}));actions.append(origin);}
      if(capabilities.orphan){
        let after=null;const find=node('button','button button-secondary',text('Tìm dữ liệu mồ côi khớp bản này','Find matching orphan payload'));find.type='button';
        find.addEventListener('click',()=>void task(async()=>{
          const owner=revision,result=await bridge.request('save_orphan_preview',{save_id:save.id,after},120000);if(owner!==revision||!container.isConnected||!row.isConnected)return;
          if(!Array.isArray(result?.matches)||result.matches.length>64||!Number.isSafeInteger(result.scanned)||result.scanned<0||result.scanned>64||result.next_after!==null&&typeof result.next_after!=='string')throw new Error('orphan_preview');
          after=result.next_after;find.textContent=after?text('Tìm nhóm mồ côi tiếp theo','Search next orphan group'):text('Tìm lại từ đầu','Search from beginning');
          if(!result.matches.length){notify(text(after?'Chưa thấy dữ liệu khớp trong nhóm này; có thể tìm nhóm tiếp.':'Không thấy dữ liệu mồ côi khớp trong lượt quét này. Tất cả tệp vẫn giữ nguyên.',after?'No match in this group; search the next group.':'No matching orphan payload was found in this scan. All files remain retained.'));return;}
          const candidate=result.matches[0];if(typeof candidate.candidate_id!=='string'||!/^[a-f0-9]{64}$/.test(candidate.revision))throw new Error('orphan_candidate');
          if(!confirm(text('Đã tìm thấy payload có đúng độ dài và checksum của bản lưu này. Đóng game và tạo bản sao phục hồi? Tệp mồ côi gốc và lịch sử vẫn giữ nguyên.','A payload matches this save’s exact length and checksum. Close the game and create a recovery copy? The orphan original and history remain unchanged.')))return;
          const recovered=await bridge.request('save_orphan_recover',{save_id:save.id,candidate_id:candidate.candidate_id,revision:candidate.revision},120000);if(owner!==revision||!container.isConnected||!row.isConnected)return;if(recovered?.metadata?.id!==save.id||typeof recovered.repaired!=='boolean')throw new Error('orphan_result');notify(recovered.repaired?text('Đã phục hồi payload, giữ nguyên tệp mồ côi gốc.','Payload recovered; the orphan original is retained.'):text('Payload hiện tại đã toàn vẹn.','The current payload is already intact.'));await load();
        }));actions.append(find);
      }
      if(capabilities.repair){
        const repair=node('button','button button-secondary',text('Khôi phục từ tệp sao lưu','Recover from backup'));repair.type='button';
        repair.addEventListener('click',()=>void task(async()=>{
          if(!confirm(text('Dừng game rồi chọn tệp đã xuất của đúng lần lưu này để khôi phục dữ liệu nếu tệp trên thiết bị bị thiếu hoặc hỏng.','Stop the game, then choose the previously exported backup of this exact saved generation to recover missing or damaged local data.')))return;
          const owner=revision,grant=await bridge.request('file_select_import',undefined,120000);
          try{
            if(owner!==revision||!container.isConnected||!row.isConnected)return;
            const result=await bridge.request('save_repair',{save_id:save.id,grant_id:grant.grant_id},120000);
            if(owner!==revision||!container.isConnected||!row.isConnected)return;
            if(result?.metadata?.id!==save.id||typeof result.repaired!=='boolean')throw new Error(text('Phản hồi khôi phục không hợp lệ.','The recovery response is invalid.'));
            notify(result.repaired?text('Đã khôi phục dữ liệu của lần lưu này.','This saved generation has been recovered.'):text('Dữ liệu hiện tại đã đầy đủ và đúng.','This saved generation is already complete and intact.'));await load();
          }finally{await bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});}
        }));actions.append(repair);
      }
      if(cloudEnabled){
        const recover=node('button','button button-secondary',text('Khôi phục từ đồng bộ','Recover from sync'));recover.type='button';
        recover.addEventListener('click',()=>void task(async()=>{
          if(!confirm(text('Kiểm tra bản lưu này và tải lại từ tài khoản đồng bộ nếu dữ liệu bị thiếu hoặc hỏng? Hãy đóng game trước.','Check this saved generation and download its copy from the connected account if local data is missing or damaged? Close the game first.')))return;
          const owner=revision,queued=await bridge.request('sync_save_recovery_request',{save_id:save.id});
          if(queued?.save_id!==save.id||queued.state!=='queued')throw new Error(text('Phản hồi yêu cầu khôi phục không hợp lệ.','The recovery request response is invalid.'));
          try{await bridge.request('sync_run_start');}catch(error){if(error.code!=='busy')throw error;}
          if(owner!==revision||!container.isConnected||!row.isConnected)return;
          notify(text('Đã xếp lịch khôi phục. Làm mới lịch sử để xem kết quả; trạng thái đồng bộ hiển thị tiến độ tải.','Recovery is queued. Refresh history for its result; synchronization status shows download progress.'));await load();
        }));actions.append(recover);
      }
      if(cloud&&['queued','downloading','waiting_for_game_close'].includes(cloud.state)){
        const cancel=node('button','button button-quiet',text('Hủy yêu cầu khôi phục','Cancel recovery'));cancel.type='button';
        cancel.addEventListener('click',()=>void task(async()=>{const owner=revision,result=await bridge.request('sync_save_recovery_cancel',{save_id:save.id});if(owner!==revision||!container.isConnected||!row.isConnected)return;notify(result?.state==='cancelled'?text('Đã hủy yêu cầu khôi phục.','The recovery request was cancelled.'):text('Yêu cầu đã kết thúc.','The recovery request has already finished.'));await load();}));actions.append(cancel);
      }
      if(entry.current_head_id!==save.id){const promote=node('button','button button-secondary',text('Dùng bản này','Use this progress'));promote.type='button';promote.addEventListener('click',()=>void task(async()=>{if(!confirm(text('Tiếp tục từ bản này trong ô lưu tương ứng? Lịch sử trước đó vẫn được giữ.','Use this progress in its save slot? Previous history remains available.')))return;await bridge.request('save_promote',{save_id:save.id,expected_head_id:entry.current_head_id});await load();}));actions.append(promote);}
      row.append(description,actions);rows.append(row);
    }
  }
  previous.addEventListener('click',()=>void task(async()=>{pageIndex--;try{await load();}catch(error){pageIndex++;throw error;}container.scrollIntoView({block:'nearest'});}));
  next.addEventListener('click',()=>void task(async()=>{const target=following;if(!target)return;cursors.splice(pageIndex+1,cursors.length,target);pageIndex++;try{await load();}catch(error){pageIndex--;throw error;}container.scrollIntoView({block:'nearest'});}));
  backup.addEventListener('click',()=>void task(async()=>{
    const ids=[...pageIds],owner=revision;if(!ids.length)return;
    const grant=await bridge.request('file_select_export',{suggested_name:`vlikeon-saves-${game.id}-page-${pageIndex+1}.opbackup`},120000);
    try{if(owner!==revision||!container.isConnected)return;const result=await bridge.request('save_backup_export',{save_ids:ids,destination_grant_id:grant.grant_id},120000);if(owner!==revision||!container.isConnected)return;if(result?.exported!==true||result.entries!==ids.length)throw new Error(text('Phản hồi sao lưu không hợp lệ.','The backup response is invalid.'));notify(exportMessage(result)+' '+text(`Gói gồm ${ids.length} bản lưu của trang này; các trang khác cần sao lưu riêng.`,`The archive includes ${ids.length} saves from this page; back up other pages separately.`));}finally{await bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});}
  }));
  restore.addEventListener('click',()=>void task(async()=>{
    if(!confirm(text('Đóng game rồi chọn gói tối đa 64 bản lưu / 128 MiB. Tất cả bản hợp lệ được khôi phục thành nhánh riêng cho game có nội dung tương ứng trong thư viện; không thay bản đang dùng. Kết quả có thể hoàn thành một phần.','Close games, then select an archive of at most 64 saves / 128 MiB. Valid saves become separate branches for matching library content; current progress stays selected. Restoration can partially complete.')))return;
    const owner=revision,grant=await bridge.request('file_select_import',undefined,120000);
    try{if(owner!==revision||!container.isConnected)return;const result=await bridge.request('save_backup_restore',{grant_id:grant.grant_id},120000);if(owner!==revision||!container.isConnected)return;if(!Array.isArray(result?.entries)||!result.entries.length||result.entries.length>64)throw new Error(text('Phản hồi khôi phục không hợp lệ.','The restore response is invalid.'));const success=result.entries.filter(entry=>entry.restored_id&&entry.error===null).length,retained=result.entries.filter(entry=>entry.already_present===true).length;notify(text(`${success}/${result.entries.length} bản đã sẵn sàng, gồm ${retained} bản đã khôi phục trước đó. Các bản thất bại chưa được thêm; tệp sao lưu vẫn giữ nguyên.`,`${success}/${result.entries.length} saves are available, including ${retained} previously restored saves. Failed entries were not added; the backup file remains unchanged.`),success!==result.entries.length);await load(true);}finally{await bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});}
  }));
  refresh.addEventListener('click',()=>void task(()=>load(true)));await task(()=>load());
}

function recoveryRecords(records){
  const uuid=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
  if(!Array.isArray(records)||records.length>64)throw new Error('Invalid cloud recovery status.');
  const result=new Map();
  for(const value of records){if(!value||!uuid.test(value.save_id)||result.has(value.save_id)||!['queued','downloading','waiting_for_game_close','recovered','already_intact','failed','cancelled'].includes(value.state)||!Number.isSafeInteger(value.requested_at_ms)||!Number.isSafeInteger(value.updated_at_ms)||value.requested_at_ms<0||value.updated_at_ms<value.requested_at_ms)throw new Error('Invalid cloud recovery status.');result.set(value.save_id,value);}
  return result;
}
function recoveryLabel(value){
  if(value.state==='failed'&&value.error?.code==='not_found')return text('Tài khoản này chưa có bản sao để khôi phục lần lưu này.','This account has no available recovery copy of this saved generation.');
  if(value.state==='failed'&&['content_mismatch','conflict','corrupt_data'].includes(value.error?.code))return text('Nguồn khôi phục không khớp hoặc không còn toàn vẹn. Dữ liệu hiện có vẫn được giữ.','The recovery source does not match or failed integrity checks. Existing local progress is retained.');
  return{
  queued:text('Đang chờ khôi phục từ đồng bộ.','Waiting for cloud recovery.'),downloading:text('Đang tải bản lưu để khôi phục.','Downloading the recovery copy.'),
  waiting_for_game_close:text('Đóng game rồi đồng bộ lại để khôi phục.','Close the game and synchronize again to recover this save.'),
  recovered:text('Đã khôi phục dữ liệu từ đồng bộ.','Payload recovered from synchronization.'),already_intact:text('Lần kiểm tra gần nhất: dữ liệu đầy đủ và đúng.','Last recovery check: payload was intact.'),
  failed:text('Chưa khôi phục được. Kiểm tra kết nối, quyền thiết bị và dung lượng rồi thử lại.','Recovery did not finish. Review the connection, device access and free storage before retrying.'),
  cancelled:text('Đã hủy yêu cầu khôi phục.','Recovery request cancelled.'),
}[value.state];}

function showOrigin(value,save){
  const dialog=node('dialog','sync-dialog save-origin-dialog');dialog.append(node('h2','',text('Nguồn gốc bản lưu','Saved progress origin')));
  const labels={captured:text('Tạo khi lưu tiến trình trên thiết bị','Created when saving progress on a device'),authenticated_remote:text('Nhận qua đồng bộ mã hóa','Received through encrypted synchronization'),imported:text('Nhập từ tệp','Imported from a file'),legacy_projection:text('Bản lưu từ phiên bản trước; chưa có đầy đủ thông tin nguồn.','Saved by an earlier version; complete origin details are unavailable.')};
  dialog.append(node('p','',labels[value.kind]));
  if(value.canonical_metadata){
    const original=value.canonical_metadata,date=new Date(original.created_at_ms),when=Number.isFinite(date.getTime())?date.toLocaleString(currentLocale()):text('Thời điểm chưa xác định','Unknown time');
    dialog.append(node('p','muted small',`${when} · ${original.core_id} ${original.core_version}`));
    if(original.generation!==save.generation)dialog.append(node('p','muted small',text(`Lần lưu tại nguồn: ${original.generation}. Thứ tự trên thiết bị này: ${save.generation}.`,`Source generation: ${original.generation}. Order on this device: ${save.generation}.`)));
    if(value.missing_parent)dialog.append(node('p','muted small',text('Bản lưu trước trong chuỗi này chưa có trên thiết bị. Liên kết nguồn vẫn được giữ.','The previous save in this chain is not on this device. Its original link is retained.')));
    if(value.imported_from)dialog.append(node('p','muted small',text('Tệp nhập được giữ làm thông tin tham chiếu. Bản lưu này bắt đầu một nhánh riêng.','The imported file metadata is retained as a reference. This save starts a separate branch.')));
    const code=node('p','muted small origin-fingerprint',text('Dấu đối chiếu: ','Origin fingerprint: ')+value.canonical_hash);dialog.append(code);
  }
  const close=node('button','button button-primary',text('Đóng','Close'));close.type='button';close.addEventListener('click',()=>dialog.close());dialog.append(close);dialog.addEventListener('close',()=>dialog.remove());document.body.append(dialog);dialog.showModal();close.focus();
}
