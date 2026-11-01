import{currentLocale,exportMessage}from'./i18n.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,label)=>{const value=document.createElement(tag);if(label!==undefined)value.textContent=label;return value;};
export class LibraryRecoveryPanel{
  #bridge;#root;#notify;#busy=false;#closed=false;#page=null;#after=null;#abort=new AbortController();
  constructor(bridge,root,notify){this.#bridge=bridge;this.#root=root;this.#notify=notify;root.hidden=true;window.addEventListener('pagehide',()=>{this.#closed=true;this.#abort.abort();},{once:true,signal:this.#abort.signal});document.addEventListener('omniplay_locale_changed',()=>this.#render(),{signal:this.#abort.signal});void bridge.request('hello').then(value=>{if(!this.#closed&&value.library_recovery_protocol===1){root.hidden=false;this.#render();}}).catch(()=>{});}
  #render(){
    if(this.#closed)return;const section=node('section');section.className='panel';section.append(node('h2',text('Phục hồi nội dung còn giữ','Recover retained content')),node('p',text('Nhập lại nội dung đã bỏ khỏi thư viện hoặc bản sao import bị gián đoạn. Chọn đúng hệ máy và tên game. Tệp cũ được giữ nguyên. Danh sách cũng có thể gồm nội dung vẫn đang dùng; nhập lại sẽ dùng mục đã có. Không tự dựng lại đĩa nhiều tệp hoặc bản lưu thiếu thông tin gốc.','Reimport retained content or interrupted raw-import snapshots. Choose the correct system and title. Existing files stay unchanged. The list may include content still in use; reimport reuses its library entry. Multi-file discs and saved progress missing its identifying details are not reconstructed.')));
    const action=(label,body)=>{const button=node('button',label);button.type='button';button.className='button button-secondary';button.disabled=this.#busy;button.addEventListener('click',()=>void this.#run(body));return button;};
    section.append(action(text('Xem nội dung có thể phục hồi','Inspect recoverable content'),()=>this.#load(null)));
    if(this.#page){section.append(node('p',text(`${this.#page.candidates.length} ứng viên · ${this.#page.retained_unknown} mục chưa xác định vẫn giữ nguyên.`,`${this.#page.candidates.length} candidates · ${this.#page.retained_unknown} unknown entries remain retained.`)));
      for(const candidate of this.#page.candidates){
        const row=node('div');row.className='save-item';const title=node('input');title.type='text';title.maxLength=120;title.placeholder=text('Tên game để phục hồi','Game title to recover');title.setAttribute('aria-label',title.placeholder);
        const system=node('select');system.setAttribute('aria-label',text('Hệ máy gốc','Original system'));for(const value of ['nes','snes','gb','gbc','gba','n64','nds','ps1','psp','mega_drive']){const option=node('option',value.toUpperCase().replaceAll('_',' '));option.value=value;system.append(option);}
        row.append(node('p',text(candidate.interrupted_import?'Bản sao import bị gián đoạn':'Nội dung trong kho',candidate.interrupted_import?'Interrupted import snapshot':'Retained content')+` · ${(candidate.bytes/1024**2).toFixed(1)} MiB · ${candidate.id.slice(0,22)}…`),title,system,action(text('Nhập lại bản này','Reimport this copy'),async()=>{
          if(!title.value.trim())throw new Error(text('Nhập tên game và chọn đúng hệ máy trước.','Enter a title and select the original system first.'));
          const result=await this.#bridge.request('library_recovery_import',{candidate_id:candidate.id,revision:candidate.revision,title:title.value.trim(),system:system.value},120000);if(this.#closed)return;this.#notify(text(`Đã thêm hoặc tìm lại: ${result.title}. Tệp phục hồi vẫn giữ nguyên.`,`Added or reused: ${result.title}. The recovery source remains retained.`));document.dispatchEvent(new CustomEvent('omniplay_library_changed'));await this.#load(this.#after);
        }));for(const field of[title,system])field.disabled=this.#busy;section.append(row);
      }
      if(this.#page.next_after)section.append(action(text('Nhóm tiếp theo','Next group'),()=>this.#load(this.#page.next_after)));
    }this.#root.replaceChildren(section);
  }
  async #load(after){const value=await this.#bridge.request('library_recovery_list',{after},30000);if(this.#closed)return;const id=value=>typeof value==='string'&&/^(?:content:[a-f0-9]{64}|pending:[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})$/.test(value);if(!Array.isArray(value?.candidates)||value.candidates.length>64||!Number.isSafeInteger(value.retained_unknown)||value.retained_unknown<0||value.next_after!==null&&!id(value.next_after))throw new Error('recovery_page');const seen=new Set();let last=after;for(const candidate of value.candidates){if(!id(candidate.id)||seen.has(candidate.id)||last!==null&&candidate.id<=last||!/^[a-f0-9]{64}$/.test(candidate.revision)||!Number.isSafeInteger(candidate.bytes)||candidate.bytes<=0||typeof candidate.interrupted_import!=='boolean')throw new Error('recovery_candidate');seen.add(candidate.id);last=candidate.id;}if(value.next_after!==null&&(value.candidates.length!==64||value.next_after!==last))throw new Error('recovery_cursor');this.#page=value;this.#after=after;}
  async #run(body){if(this.#busy||this.#closed)return;this.#busy=true;this.#render();try{await body();}catch(error){if(!this.#closed)this.#notify(error.message||text('Chưa phục hồi được; tệp vẫn được giữ.','Recovery did not finish; the source remains retained.'),true);}finally{this.#busy=false;this.#render();}}
}

export class ProfileBackupAndRecoveryPanel{
  #bridge;#root;#notify;#onProfileChanged;#caps={backup:false,db:false,reconstruct:false};
  #includeRoms=false;#inspectedBackup=null;#lastRestore=null;#dbHealth=null;#dbOutcome=null;#reconstructPreview=null;
  #busy=false;#closed=false;#abort=new AbortController();
  constructor(bridge,root,notify,onProfileChanged){
    this.#bridge=bridge;this.#root=root;this.#notify=notify;this.#onProfileChanged=onProfileChanged;root.hidden=true;
    window.addEventListener('pagehide',()=>{this.#closed=true;this.#abort.abort();},{once:true,signal:this.#abort.signal});
    document.addEventListener('omniplay_locale_changed',()=>this.#render(),{signal:this.#abort.signal});
    void bridge.request('hello').then(hello=>{
      if(this.#closed)return;
      this.#caps={
        backup:hello.profile_backup_protocol===1,
        db:hello.database_recovery_protocol===1,
        reconstruct:hello.library_reconstruct_protocol===1,
      };
      if(this.#caps.backup||this.#caps.db||this.#caps.reconstruct){root.hidden=false;this.#render();}
    }).catch(()=>{});
  }
  #action(label,body,disabled=false){
    const button=node('button',label);button.type='button';button.className='button button-secondary';
    button.disabled=this.#busy||disabled;button.addEventListener('click',()=>void this.#run(body));return button;
  }
  #render(){
    if(this.#closed)return;this.#root.replaceChildren();
    if(this.#caps.backup){
      const sec=node('section');sec.className='panel';
      sec.append(node('h2',text('Sao lưu & khôi phục toàn bộ hồ sơ (.opprofile)','Full profile backup & restore (.opprofile)')),node('p',text('Xuất, kiểm tra hoặc khôi phục gói OPPF0001 (tối đa 16 GiB) gồm cài đặt, lịch sử save đa thế hệ, firmware và (tùy chọn) tệp ROM/đĩa. Toàn bộ gói và SHA-256 được xác minh trước khi ghi cục bộ; hạn mức vật lý cục bộ không bị tự nâng khi khôi phục.','Export, inspect, or restore an OPPF0001 archive (up to 16 GiB) containing settings, multi-generation saves, firmware, and optional ROM/disc assets. Checksums and archive framing are verified before mutating local state, and local physical quota is preserved.')));
      const toggleLabel=node('label');const toggle=node('input');toggle.type='checkbox';toggle.checked=this.#includeRoms;toggle.disabled=this.#busy;
      toggle.addEventListener('change',()=>{this.#includeRoms=toggle.checked;});
      toggleLabel.append(toggle,node('span',text(' Đính kèm tệp ROM/đĩa vào bản sao lưu',' Include ROM and disc assets in backup')));
      const actions=node('div');actions.className='detail-actions';
      actions.append(
        this.#action(text('Xuất hồ sơ (.opprofile)','Export profile (.opprofile)'),()=>this.#exportProfile()),
        this.#action(text('Kiểm tra gói sao lưu','Inspect backup archive'),()=>this.#inspectProfile()),
        this.#action(text('Khôi phục hồ sơ','Restore profile archive'),()=>this.#restoreProfile())
      );
      sec.append(toggleLabel,actions);
      if(this.#inspectedBackup){
        const b=this.#inspectedBackup;
        sec.append(node('p',text(`Gói hợp lệ · ${b.games} game · ${b.saves} bản lưu · ${b.firmware_assets} firmware · ROM đính kèm: ${b.include_rom_assets?'Có':'Không'} · ${(Number(b.total_payload_bytes||0)/(1024*1024)).toFixed(1)} MiB`,`Verified archive · ${b.games} games · ${b.saves} saves · ${b.firmware_assets} firmware · Includes ROMs: ${b.include_rom_assets?'Yes':'No'} · ${(Number(b.total_payload_bytes||0)/(1024*1024)).toFixed(1)} MiB`)));
      }
      if(this.#lastRestore){
        const r=this.#lastRestore;
        sec.append(node('p',text(`Đã khôi phục: ${r.games_restored} game mới · ${r.saves_restored} save mới (${r.saves_already_present} đã có, ${r.saves_skipped} bỏ qua, ${r.saves_failed??0} lỗi) · ${r.firmware_restored} firmware.`,`Restored: ${r.games_restored} games · ${r.saves_restored} saves (${r.saves_already_present} already present, ${r.saves_skipped} skipped, ${r.saves_failed??0} failed) · ${r.firmware_restored} firmware.`)));
      }
      this.#root.append(sec);
    }
    if(this.#caps.db){
      const sec=node('section');sec.className='panel';
      sec.append(node('h2',text('Chẩn đoán & khôi phục cơ sở dữ liệu hồ sơ','Profile database health & corruption recovery')),node('p',text('Kiểm tra tính toàn vẹn SQLite (omniplay.sqlite3). Khi phát hiện hư hỏng, tệp gốc được cách ly nguyên vẹn kèm biên nhận SHA-256 trước khi dựng lại schema 15 và cứu dữ liệu hợp lệ.','Checks SQLite integrity (omniplay.sqlite3). When corruption is detected, damaged files are quarantined intact with a SHA-256 receipt before rebuilding a clean schema-15 database and salvaging verified rows.')));
      const actions=node('div');actions.className='detail-actions';
      actions.append(
        this.#action(text('Kiểm tra sức khỏe DB','Check database health'),async()=>{this.#dbHealth=await this.#bridge.request('database_health_check',undefined,30000);}),
        this.#action(text('Cách ly & khôi phục DB hỏng','Quarantine & recover corrupt DB'),async()=>{
          if(!window.confirm(text('Cách ly cơ sở dữ liệu hỏng sang corrupt-db-quarantine và dựng lại hồ sơ từ dữ liệu hợp lệ?','Quarantine the damaged database into corrupt-db-quarantine and rebuild from verified rows and assets?')))return;
          this.#dbOutcome=await this.#bridge.request('database_recover_corrupt',undefined,120000);
          this.#dbHealth=await this.#bridge.request('database_health_check',undefined,30000);
          if(this.#onProfileChanged)await this.#onProfileChanged();
          document.dispatchEvent(new CustomEvent('omniplay_library_changed'));
          this.#notify(text(`Đã khôi phục DB (mã cách ly: ${this.#dbOutcome.quarantine_id}).`,`Recovered database (quarantine ID: ${this.#dbOutcome.quarantine_id}).`));
        },!this.#dbHealth?.recoverable)
      );
      sec.append(actions);
      if(this.#dbHealth){
        const h=this.#dbHealth;
        sec.append(node('p',text(`Trạng thái: ${h.status} · Schema: ${h.user_version??'—'} · Dung lượng: ${h.db_bytes} B · Khả năng khôi phục: ${h.recoverable?'Có':'Không'}`,`Status: ${h.status} · Schema: ${h.user_version??'—'} · Size: ${h.db_bytes} B · Recoverable: ${h.recoverable?'Yes':'No'}`)));
        if(Array.isArray(h.findings)&&h.findings.length)sec.append(node('p',h.findings.join(' · ')));
      }
      if(this.#dbOutcome){
        const o=this.#dbOutcome;
        sec.append(node('p',text(`Cách ly ${o.quarantine_id}: cứu ${o.salvaged_games} game, dựng lại ${o.reconstructed_games} game, cứu ${o.salvaged_saves} save.`,`Quarantined ${o.quarantine_id}: salvaged ${o.salvaged_games} games, reconstructed ${o.reconstructed_games} games, salvaged ${o.salvaged_saves} saves.`)));
      }
      this.#root.append(sec);
    }
    if(this.#caps.reconstruct){
      const sec=node('section');sec.className='panel';
      sec.append(node('h2',text('Tái dựng metadata game & bộ đĩa nhiều tệp (.cue / .m3u)','Reconstruct game metadata & multi-file discs (.cue / .m3u)')),node('p',text('Quét các blob chưa có trong mục lục thư viện tại assets/, chuẩn hóa lại bảng mục lục .cue/.m3u và liên kết track đĩa mà không sửa tệp gốc.','Scans unindexed blobs in assets/, normalizes .cue/.m3u descriptors, and binds multi-track discs without modifying retained blobs.')));
      sec.append(this.#action(text('Quét ứng viên tái dựng','Scan reconstruction candidates'),async()=>{this.#reconstructPreview=await this.#bridge.request('library_reconstruct_preview',undefined,60000);}));
      const p=this.#reconstructPreview;
      if(p){
        const tracks=Array.isArray(p.available_track_blobs)?p.available_track_blobs:[];
        sec.append(node('p',text(`${p.candidates?.length??0} ứng viên tái dựng · ${tracks.length} blob track rời · ${p.already_indexed_blobs??0} blob đã có trong thư viện.`,`${p.candidates?.length??0} reconstruction candidates · ${tracks.length} unassigned track blobs · ${p.already_indexed_blobs??0} indexed blobs.`)));
        for(const cand of(p.candidates||[])){
          const row=node('div');row.className='save-item';
          const title=node('input');title.type='text';title.maxLength=150;title.value=cand.suggested_title||'';title.disabled=this.#busy;
          title.setAttribute('aria-label',text('Tên game tái dựng','Reconstructed game title'));
          const sys=node('select');sys.disabled=this.#busy;sys.setAttribute('aria-label',text('Hệ máy','System'));
          for(const value of['ps1','psp','nes','snes','gb','gbc','gba','n64','nds','mega_drive']){
            const opt=node('option',value.toUpperCase().replaceAll('_',' '));opt.value=value;
            if(cand.suggested_system===value)opt.selected=true;
            sys.append(opt);
          }
          const trackSelectors=new Map();
          row.append(node('p',`${cand.format} · ${cand.disc_count} disc · ${(Number(cand.total_bytes_known||0)/(1024*1024)).toFixed(1)} MiB · ${cand.entry_hash.slice(0,16)}…`),title,sys);
          if(cand.needs_track_binding&&Array.isArray(cand.required_tracks)){
            for(const reqTrack of cand.required_tracks){
              const prebound=cand.resolved_bindings?.[reqTrack];
              if(prebound)continue;
              const sel=node('select');sel.disabled=this.#busy;sel.setAttribute('aria-label',`Track ${reqTrack}`);
              for(const blob of tracks){
                const opt=node('option',`${reqTrack} → ${blob.content_hash.slice(0,16)}… (${(blob.byte_length/(1024*1024)).toFixed(1)} MiB ${blob.detected_kind})`);
                opt.value=blob.content_hash;sel.append(opt);
              }
              trackSelectors.set(reqTrack,sel);
              row.append(sel);
            }
          }
          row.append(this.#action(text('Tái dựng vào thư viện','Reconstruct into library'),async()=>{
            let bindings=null;
            if(cand.needs_track_binding){
              bindings={...(cand.resolved_bindings||{})};
              for(const[reqName,sel]of trackSelectors.entries()){
                if(!sel.value)throw new Error(text(`Chọn blob cho track ${reqName}.`,`Select a track blob for ${reqName}.`));
                bindings[reqName]=sel.value;
              }
            }
            const manifest=await this.#bridge.request('library_reconstruct_commit',{
              candidate_id:cand.candidate_id,
              revision:cand.revision,
              title:title.value.trim()||null,
              system:sys.value,
              track_bindings:bindings,
            },120000);
            this.#notify(text(`Đã tái dựng: ${manifest.title}`,`Reconstructed: ${manifest.title}`));
            document.dispatchEvent(new CustomEvent('omniplay_library_changed'));
            this.#reconstructPreview=await this.#bridge.request('library_reconstruct_preview',undefined,60000);
          }));
          sec.append(row);
        }
      }
      this.#root.append(sec);
    }
  }
  async #exportProfile(){
    const grant=await this.#bridge.request('file_select_export',{suggested_name:`omniplay-profile-${new Date().toISOString().slice(0,10)}.opprofile`},120000);
    try{
      if(this.#closed)return;
      const res=await this.#bridge.request('profile_backup_export',{include_rom_assets:this.#includeRoms,destination_grant_id:grant.grant_id},120000);
      if(!this.#closed)this.#notify(exportMessage(res));
    }finally{await this.#bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});}
  }
  async #inspectProfile(){
    const grant=await this.#bridge.request('profile_backup_select_import',undefined,120000);
    let stagedToken=null;
    try{
      if(this.#closed)return;
      const summary=await this.#bridge.request('profile_backup_inspect',{grant_id:grant.grant_id},120000);
      stagedToken=summary?.restore_token||null;
      if(!this.#closed)this.#inspectedBackup=summary?{...summary,restore_token:null}:null;
    }finally{
      if(stagedToken)await this.#bridge.request('profile_backup_cancel',{restore_token:stagedToken}).catch(()=>{});
      await this.#bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});
    }
  }
  async #restoreProfile(){
    const grant=await this.#bridge.request('profile_backup_select_import',undefined,120000);
    let restoreToken=null;
    try{
      if(this.#closed)return;
      const summary=await this.#bridge.request('profile_backup_inspect',{grant_id:grant.grant_id},120000);
      restoreToken=summary?.restore_token||null;
      if(this.#closed)return;
      this.#inspectedBackup=summary?{...summary,restore_token:null}:null;
      if(!restoreToken||typeof summary?.archive_hash!=='string'||!/^[a-f0-9]{64}$/.test(summary.archive_hash)){
        throw new Error(text('Không nhận được mã xác nhận khôi phục hợp lệ cho gói đã kiểm tra.','Missing verified restore token for the inspected archive.'));
      }
      const shortHash=summary.archive_hash.slice(0,12);
      if(!window.confirm(text(`Khôi phục gói sao lưu đã xác minh (${shortHash}…) gồm ${summary.games} game và ${summary.saves} bản lưu vào hồ sơ hiện tại?`,`Restore verified backup archive (${shortHash}…) with ${summary.games} games and ${summary.saves} saves into the current profile?`)))return;
      const confirmedToken=restoreToken;restoreToken=null;
      const restored=await this.#bridge.request('profile_backup_restore',{restore_token:confirmedToken,expected_archive_hash:summary.archive_hash},120000);
      if(this.#closed)return;
      this.#lastRestore=restored;
      if(this.#onProfileChanged)await this.#onProfileChanged();
      document.dispatchEvent(new CustomEvent('omniplay_library_changed'));
      this.#notify(text(`Đã khôi phục hồ sơ (${restored.games_restored} game, ${restored.saves_restored} bản lưu).`,`Restored profile (${restored.games_restored} games, ${restored.saves_restored} saves).`));
    }finally{
      if(restoreToken)await this.#bridge.request('profile_backup_cancel',{restore_token:restoreToken}).catch(()=>{});
      await this.#bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});
    }
  }
  async #run(body){
    if(this.#busy||this.#closed)return;this.#busy=true;this.#render();
    try{await body();}catch(error){if(!this.#closed&&error?.code!=='cancelled')this.#notify(error?.message||text('Thao tác sao lưu/phục hồi chưa hoàn tất.','Backup or recovery operation could not finish.'),true);}
    finally{this.#busy=false;this.#render();}
  }
}
