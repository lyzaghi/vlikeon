import{currentLocale}from'./i18n.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,label)=>{const value=document.createElement(tag);if(label!==undefined)value.textContent=label;return value;};
const count=value=>Number.isSafeInteger(value)&&value>=0;
const hash=value=>typeof value==='string'&&/^[a-f0-9]{64}$/.test(value);
export class UnusedContentPanel{
  #bridge;#root;#notify;#preview=null;#busy=false;#closed=false;#abort=new AbortController();
  constructor(bridge,root,notify){
    this.#bridge=bridge;this.#root=root;this.#notify=notify;root.hidden=true;
    window.addEventListener('pagehide',()=>{this.#closed=true;this.#abort.abort();},{once:true,signal:this.#abort.signal});
    document.addEventListener('omniplay_locale_changed',()=>this.#render(),{signal:this.#abort.signal});
    void bridge.request('hello').then(value=>{if(this.#closed||value.storage_unused_protocol!==1)return;root.hidden=false;this.#render();}).catch(()=>{});
  }
  #render(){
    if(this.#closed)return;this.#root.replaceChildren();const section=node('section');section.className='panel';
    section.append(node('h2',text('Nội dung game không còn sử dụng','Unused game content')),node('p',text('Gỡ game khỏi thư viện giữ lại các tệp nội dung để phục hồi. Chỉ dọn từng nhóm sau khi xem trước; bạn cần nhập lại tệp gốc nếu muốn chơi lại. Bản lưu, firmware và tệp gốc của bạn không nằm trong nhóm dọn này.','Removing a library entry retains its content for recovery. Review each group before cleanup; playing removed games again requires importing the original files. Saves, firmware and your original files are outside this cleanup.')));
    const status=node('p');status.setAttribute('role','status');const preview=this.#preview;
    status.textContent=this.#busy?text('Đang kiểm tra quyền sở hữu nội dung…','Checking content ownership…'):preview?text(`${preview.unused_assets}/${preview.scanned_assets} tệp có thể dọn · ${(preview.unused_bytes/1024**2).toFixed(1)} MiB nội dung.`,`${preview.unused_assets}/${preview.scanned_assets} files can be removed · ${(preview.unused_bytes/1024**2).toFixed(1)} MiB of content.`):text('Đóng game và xem trước nhóm nội dung trước khi dọn.','Close games and preview a content group before cleanup.');
    const action=(label,callback)=>{const button=node('button',label);button.type='button';button.className='button button-secondary';button.disabled=this.#busy;button.addEventListener('click',()=>void this.#run(callback));return button;};
    section.append(status,action(text('Làm mới từ đầu','Refresh from beginning'),()=>this.#load(null)));
    if(preview?.retained_unknown_entries)section.append(node('p',text(`${preview.retained_unknown_entries} mục chưa xác định được giữ nguyên để phục hồi; không nằm trong nhóm dọn.`,`${preview.retained_unknown_entries} unknown entries are retained for recovery and excluded from cleanup.`)));
    if(preview?.more_available)section.append(action(text('Nhóm tiếp theo','Next group'),()=>this.#load(preview.next_after)));
    if(preview?.unused_assets)section.append(action(text('Dọn nhóm đã xem','Remove reviewed group'),async()=>{
      if(!window.confirm(text(`Gỡ ${preview.unused_assets} tệp nội dung không còn được dùng (${(preview.unused_bytes/1024**2).toFixed(1)} MiB)? Cần tệp game gốc để nhập lại.`,`Remove ${preview.unused_assets} unused content files (${(preview.unused_bytes/1024**2).toFixed(1)} MiB)? You will need the original game files to import them again.`)))return;
      const result=await this.#bridge.request('storage_unused_remove',{after_hash:preview.after_hash,revision:preview.revision},120000);
      if(this.#closed)return;
      this.#preview=null;
      if(!count(result?.removed_assets)||result.removed_assets>preview.unused_assets||!count(result?.removed_blob_reference_bytes)||result.removed_blob_reference_bytes>preview.unused_bytes||!count(result?.retained_assets)||result.removed_assets+result.retained_assets!==preview.unused_assets||typeof result.durability_confirmed!=='boolean')throw new Error('cleanup_result');
      const retained=result.retained_assets?text(` ${result.retained_assets} tệp chưa gỡ được, vẫn giữ lại.`,` ${result.retained_assets} files could not be removed and remain retained.`):'';
      const durability=result.removed_assets&&!result.durability_confirmed?text(' Chưa xác nhận độ bền lưu trữ; hãy làm mới để kiểm tra.',' Storage durability is unconfirmed; refresh to inspect.'):'';
      this.#notify(text(`Đã gỡ ${result.removed_assets} tệp. Làm mới để xem phần còn lại.`,`Removed ${result.removed_assets} files. Refresh to inspect remaining content.`)+retained+durability,Boolean(result.retained_assets));
    }));
    this.#root.append(section);
  }
  async #load(after){
    const value=await this.#bridge.request('storage_unused_preview',{after_hash:after},120000);if(this.#closed)return;
    if(value?.schema_version!==1||!hash(value.revision)||value.after_hash!==after||!count(value.scanned_assets)||value.scanned_assets>64||!count(value.unused_assets)||value.unused_assets>value.scanned_assets||!count(value.unused_bytes)||!count(value.retained_unknown_entries)||!count(value.library_games)||typeof value.more_available!=='boolean'||(value.more_available?!hash(value.next_after)||value.scanned_assets!==64||after!==null&&value.next_after<=after:value.next_after!==null))throw new Error('cleanup_preview');
    this.#preview=value;
  }
  async #run(body){
    if(this.#busy||this.#closed)return;this.#busy=true;this.#render();
    try{await body();}catch{this.#preview=null;if(!this.#closed)this.#notify(text('Chưa hoàn tất. Đóng game, đợi import kết thúc rồi làm mới; các tệp chưa xử lý được giữ lại.','This could not finish. Close games, wait for imports and refresh; unprocessed files remain retained.'),true);}
    finally{this.#busy=false;this.#render();}
  }
}

export class ProfileStorageQuotaPanel{
  #bridge;#root;#notify;#report=null;#busy=false;#closed=false;#abort=new AbortController();
  constructor(bridge,root,notify){
    this.#bridge=bridge;this.#root=root;this.#notify=notify;root.hidden=true;
    window.addEventListener('pagehide',()=>{this.#closed=true;this.#abort.abort();},{once:true,signal:this.#abort.signal});
    document.addEventListener('omniplay_locale_changed',()=>this.#render(),{signal:this.#abort.signal});
    void bridge.request('hello').then(value=>{if(this.#closed||value.storage_accounting_protocol!==1)return;root.hidden=false;this.#render();}).catch(()=>{});
  }
  #render(){
    if(this.#closed)return;this.#root.replaceChildren();const section=node('section');section.className='panel';
    section.append(node('h2',text('Hạn mức vật lý & tham chiếu giữ dữ liệu','Physical storage quota & active retainers')),node('p',text('Truy vấn dung lượng cấp phát thực từ hệ điều hành theo từng vùng lưu trữ, hiển thị chất lượng số đo/cụm volume, loại trùng hard-link khi định danh tệp được xác minh, và kiểm tra các tham chiếu đang giữ blob (thư viện, phiên chơi, save, firmware, native-work, staging khôi phục).','Queries OS-reported physical allocation per namespace, surfaces measurement quality and volume cluster size, deduplicates hard-links only when file identity is verified, and inspects active retainers keeping blobs alive (library, sessions, saves, firmware, native-work, restore staging).')));
    const action=(label,callback)=>{const button=node('button',label);button.type='button';button.className='button button-secondary';button.disabled=this.#busy;button.addEventListener('click',()=>void this.#run(callback));return button;};
    section.append(action(text('Kiểm tra dung lượng vật lý','Inspect physical usage'),async()=>{this.#report=await this.#bridge.request('storage_accounting_report',undefined,60000);}));
    const r=this.#report;
    if(r){
      const mib=b=>(Number(b||0)/(1024*1024)).toFixed(1);
      const kib=b=>(Number(b||0)/1024).toFixed(1);
      const quality=r.measurement_quality||'exact_os_allocation';
      const qualityLabel=quality==='exact_os_allocation'
        ?text('Cấp phát OS chính xác','Exact OS allocation')
        :quality==='conservative_missing_identity'
          ?text('Bảo thủ (thiếu file-index, không loại trùng hard-link)','Conservative (missing file identity; hard-link dedup disabled)')
          :text('Suy giảm (có tệp không truy vấn được cấp phát OS — chặn cấp phát quota mới)','Degraded (unmeasured OS allocation — new quota admission blocked)');
      const physicalPrefix=quality==='exact_os_allocation'
        ?text('Vật lý OS đã đo','Measured OS physical')
        :text('Vật lý OS đã đo một phần (xem cảnh báo)','Partially measured OS physical (see warnings)');
      const summary=node('div');summary.className='save-item';
      summary.append(
        node('p',text(`Chất lượng số đo: ${qualityLabel} · Cụm volume: ${r.volume_cluster_bytes?`${kib(r.volume_cluster_bytes)} KiB`:'chưa xác định'}`, `Measurement quality: ${qualityLabel} · Volume cluster: ${r.volume_cluster_bytes?`${kib(r.volume_cluster_bytes)} KiB`:'unknown'}`)),
        node('p',text(`${physicalPrefix}: ${mib(r.physical_bytes)} MiB · Logic: ${mib(r.logical_bytes)} MiB · Đã giữ chỗ: ${mib(r.effective_committed_physical_bytes)} / ${mib(r.maximum_physical_bytes)} MiB · Còn trống: ${mib(r.available_physical_bytes)} MiB · Hard-link loại trùng: ${r.hardlink_deduped_files}`,`${physicalPrefix}: ${mib(r.physical_bytes)} MiB · Logical: ${mib(r.logical_bytes)} MiB · Committed: ${mib(r.effective_committed_physical_bytes)} / ${mib(r.maximum_physical_bytes)} MiB · Available: ${mib(r.available_physical_bytes)} MiB · Hard-links deduplicated: ${r.hardlink_deduped_files}`)),
        node('p',text(`Tham chiếu giữ dữ liệu: ${r.retainer_counts?.library_games??0} game · ${r.retainer_counts?.live_saves??0} save · ${r.retainer_counts?.firmware_assets??0} firmware · ${r.retainer_counts?.active_sessions??0} phiên đang mở · ${r.retainer_counts?.native_work_debts??0} native-work · ${r.retainer_counts?.profile_restore_staging_hashes??0} blob staging khôi phục`,`Active retainers: ${r.retainer_counts?.library_games??0} games · ${r.retainer_counts?.live_saves??0} saves · ${r.retainer_counts?.firmware_assets??0} firmware · ${r.retainer_counts?.active_sessions??0} active sessions · ${r.retainer_counts?.native_work_debts??0} native-work · ${r.retainer_counts?.profile_restore_staging_hashes??0} staged restore blobs`))
      );
      if(Number(r.unmeasured_files||0)>0||Number(r.unverified_identity_files||0)>0){
        summary.append(node('p',text(`Tệp chưa đo được cấp phát OS: ${r.unmeasured_files??0} (${mib(r.unmeasured_logical_bytes)} MiB logic) · Tệp thiếu file-index: ${r.unverified_identity_files??0}`,`Unmeasured OS-allocation files: ${r.unmeasured_files??0} (${mib(r.unmeasured_logical_bytes)} MiB logical) · Unverified file-identity entries: ${r.unverified_identity_files??0}`)));
      }
      if(Array.isArray(r.measurement_warnings)&&r.measurement_warnings.length){
        for(const warning of r.measurement_warnings){
          const warnNode=node('p',`⚠ ${warning}`);warnNode.setAttribute('role','alert');
          summary.append(warnNode);
        }
      }
      if(Array.isArray(r.namespaces)){
        for(const ns of r.namespaces){
          summary.append(node('p',`${ns.namespace}: ${mib(ns.physical_bytes)} MiB (${ns.file_count} files, ${ns.unreferenced_files} unreferenced)`));
        }
      }
      const quotaWrap=node('div');quotaWrap.className='detail-actions';
      const quotaInput=node('input');quotaInput.type='number';quotaInput.min='256';quotaInput.max='524288';quotaInput.step='256';
      quotaInput.value=String(Math.max(256,Math.round(Number(r.maximum_physical_bytes||0)/(1024*1024))));
      quotaInput.disabled=this.#busy;
      quotaInput.setAttribute('aria-label',text('Hạn mức vật lý tối đa (MiB, 256–524288)','Maximum physical quota (MiB, 256–524288)'));
      quotaWrap.append(quotaInput,action(text('Cập nhật hạn mức (MiB)','Update quota (MiB)'),async()=>{
        const mibVal=Number(quotaInput.value);
        if(!Number.isFinite(mibVal)||mibVal<256||mibVal>524288)throw new Error(text('Hạn mức phải từ 256 MiB đến 524288 MiB (512 GiB).','Quota must be between 256 MiB and 524288 MiB (512 GiB).'));
        await this.#bridge.request('storage_quota_set',{maximum_physical_bytes:Math.round(mibVal)*1024*1024},30000);
        this.#report=await this.#bridge.request('storage_accounting_report',undefined,60000);
        this.#notify(text('Đã cập nhật hạn mức lưu trữ vật lý.','Updated physical storage quota.'));
      }));
      summary.append(quotaWrap);
      section.append(summary);
    }
    this.#root.append(section);
  }
  async #run(body){
    if(this.#busy||this.#closed)return;this.#busy=true;this.#render();
    try{await body();}catch(error){if(!this.#closed)this.#notify(error?.message||text('Không thể cập nhật báo cáo hạn mức vật lý.','Could not update physical storage report.'),true);}
    finally{this.#busy=false;this.#render();}
  }
}
