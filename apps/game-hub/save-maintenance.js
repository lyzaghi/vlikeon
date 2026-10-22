import {currentLocale} from './i18n.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,className,value)=>{const item=document.createElement(tag);if(className)item.className=className;if(value!==undefined)item.textContent=value;return item;};
export class SaveMaintenancePanel{
  #bridge;#root;#notify;#status={pending_files:0,pending_bytes:0};#busy=false;#stop=false;#finished=false;
  constructor(bridge,root,notify){this.#bridge=bridge;this.#root=root;this.#notify=notify;document.addEventListener('omniplay_locale_changed',()=>this.#render());this.#render();window.addEventListener('pagehide',()=>{this.#stop=true;});}
  async refresh(){if(this.#busy)return;this.#status=await this.#bridge.request('save_cleanup_status');this.#render();}
  #render(){
    this.#root.replaceChildren();const section=node('section','panel save-maintenance');section.append(node('h2','',text('Dọn tệp bản lưu đã xoá','Remove retired save files')),node('p','muted small',text('Chỉ xử lý các tệp thuộc lịch sử bạn đã chọn xoá trên máy này. Bản lưu còn sử dụng, ROM và dữ liệu trên dịch vụ đồng bộ được giữ nguyên.','Processes only files from local history you chose to remove. Live saves, ROMs and cloud data keep their own controls.')));
    const status=node('p','muted small');status.setAttribute('role','status');const count=this.#status.pending_files,mb=(this.#status.pending_bytes/1024**2).toFixed(1);status.textContent=count?text(`${count} tệp đang chờ dọn · ${mb} MiB dữ liệu logic.`,`${count} files pending · ${mb} MiB of logical data.`):text('Không có tệp bản lưu nào đang chờ dọn.','No retired save files are pending.');
    if(this.#finished&&count)status.textContent+=' '+text('Một số tệp chưa xử lý được. Bạn có thể thử lại sau.','Some files could not be processed. You can retry later.');
    const button=node('button','button button-secondary',this.#busy?text('Dừng sau lượt hiện tại','Stop after this batch'):text('Tiếp tục dọn tệp','Resume cleanup'));button.type='button';button.disabled=!this.#busy&&!count;button.addEventListener('click',()=>{if(this.#busy){this.#stop=true;button.disabled=true;}else void this.run();});section.append(status,button);this.#root.append(section);
  }
  async run(){
    if(this.#busy)return;this.#busy=true;this.#stop=false;this.#finished=false;
    try{
      this.#status=await this.#bridge.request('save_cleanup_status');this.#render();const batches=Math.min(200,Math.ceil(this.#status.pending_files/128)+1);
      for(let index=0;index<batches&&!this.#stop&&this.#status.pending_files;index++){
        this.#status=await this.#bridge.request('save_cleanup',undefined,30000);this.#render();await new Promise(resolve=>setTimeout(resolve,40));
      }
      this.#finished=true;
    }catch{this.#finished=true;this.#notify(text('Chưa hoàn tất dọn tệp đã xoá. Trạng thái chờ được giữ để thử lại.','Retired-file cleanup could not finish. Pending work is retained for retry.'),true);}
    finally{this.#busy=false;this.#render();}
  }
}

export async function removeLocalGameSaves(bridge,game,notify){
  const preview=await bridge.request('save_purge_preview',{game_id:game.id},120000);
  if(!preview.save_count){notify(text('Game này không có bản lưu cục bộ để xoá.','This game has no local saved progress to remove.'));return null;}
  const size=(preview.payload_bytes/1024**2).toFixed(1);
  const title=Array.from(game.title.replace(/[\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g,' ')).slice(0,100).join('');
  const question=text(`Xoá toàn bộ ${preview.save_count} bản lưu cục bộ của “${title}” (${size} MiB), gồm cả bản đang sử dụng?\n\nHãy xuất các bản cần giữ trước khi xoá. Thao tác này không hoàn tác được trong thư viện. Dữ liệu trên dịch vụ đồng bộ và thiết bị khác không bị xoá; các bản mới vẫn có thể đến khi đồng bộ.`,`Remove all ${preview.save_count} local saves for “${title}” (${size} MiB), including current progress?\n\nExport anything you want to keep first. This cannot be undone in the library. Cloud data and other devices are not deleted; new progress may still arrive through sync.`);
  if(!window.confirm(question))return null;
  const result=await bridge.request('save_purge',{game_id:game.id,expected_revision:preview.revision},120000);
  notify(text(`Đã xoá ${result.removed_count} bản lưu cục bộ.`,`Removed ${result.removed_count} local saves.`));return result;
}
