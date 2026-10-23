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
