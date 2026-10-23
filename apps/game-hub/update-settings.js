import{currentLocale}from'./i18n.js';
const labels={
  vi:{title:'Cập nhật OmniPlay',disabled:'Bản phát triển này chưa kết nối kênh phát hành.',description:'Bản cập nhật được kiểm tra chữ ký và lưu riêng trước khi áp dụng.',download:'Tải bản mới',working:'Đang tải và chuẩn bị bản cập nhật…',cancel:'Dừng tải',activate:'Áp dụng khi mở lại',applied:'Bản cập nhật sẽ được dùng khi mở lại OmniPlay.',rollback:'Dùng lại bản trước',rollbackConfirm:'Dùng lại bản trước nếu bản đó vẫn đáp ứng yêu cầu bảo mật?',active:'Đang dùng',staged:'Sẵn sàng áp dụng',previous:'Bản trước',rejected:'Đã bỏ qua',done:'Bản cập nhật đã sẵn sàng.',restart:'Lưu game và đóng toàn bộ cửa sổ OmniPlay, sau đó mở lại để chuyển phiên bản.'},
  en:{title:'OmniPlay updates',disabled:'This development build has no release channel configured.',description:'Updates are signature-checked and staged separately before activation.',download:'Download latest release',working:'Downloading and preparing the update…',cancel:'Stop download',activate:'Use on next launch',applied:'The update will be used the next time OmniPlay starts.',rollback:'Return to the previous release',rollbackConfirm:'Return to the previous release if it still meets the security requirements?',active:'Active',staged:'Ready to apply',previous:'Previous',rejected:'Skipped',done:'The update is ready.',restart:'Save your game and close every OmniPlay window, then reopen to switch versions.'}
};
const text=k=>(labels[currentLocale()]||labels.en)[k];
function node(tag,className,value){const item=document.createElement(tag);item.className=className;if(value!==undefined)item.textContent=value;return item;}
export class UpdateSettings {
  #bridge;#container;#notify;#job=null;#timer=0;#revision=0;#starting=false;
  constructor(bridge,container,notify){this.#bridge=bridge;this.#container=container;this.#notify=notify;}
  #button(label,action){const button=node('button','button button-secondary',label);button.type='button';button.addEventListener('click',async()=>{button.disabled=true;try{await action();}catch(error){this.#notify(error.message,true);}finally{button.disabled=false;}});return button;}
  async refresh(){
    const revision=++this.#revision;
    try{
      const state=await this.#bridge.request('update_status');if(revision!==this.#revision||!this.#container.isConnected)return;
      const panel=node('section','panel sync-panel');panel.append(node('h2','',text('title')),node('p','muted small',text(state.configured?'description':'disabled')));
      if(state.configured){
        const download=this.#button(text('download'),async()=>{if(this.#starting||this.#job?.state==='running')return;this.#starting=true;try{this.#job=await this.#bridge.request('update_download_start');this.#schedule();}finally{this.#starting=false;await this.refresh();}});download.disabled=this.#starting||this.#job?.state==='running';panel.append(download);
        for(const release of state.releases){
          const row=node('div','save-item'),label=node('p','small',`${release.version} · ${text(release.status)}`);row.append(label);
          if(release.status==='staged')row.append(this.#button(text('activate'),async()=>{await this.#bridge.request('update_activate',{release_id:release.id},120000);this.#notify(`${text('applied')} ${text('restart')}`);await this.refresh();}));
          panel.append(row);
        }
        if(state.releases.some(release=>release.status==='previous'))panel.append(this.#button(text('rollback'),async()=>{if(!window.confirm(text('rollbackConfirm')))return;await this.#bridge.request('update_rollback',{},120000);this.#notify(text('restart'));await this.refresh();}));
      }
      if(this.#job?.state==='running'){const status=node('p','sync-status',text('working'));status.setAttribute('role','status');status.append(this.#button(text('cancel'),async()=>{await this.#bridge.request('update_job_cancel',{job_id:this.#job.id});}));panel.append(status);}
      this.#container.replaceChildren(panel);
    }catch(error){if(revision===this.#revision&&this.#container.isConnected)this.#container.replaceChildren(node('p','muted small',error.message));}
  }
  #schedule(){clearTimeout(this.#timer);this.#timer=setTimeout(()=>void this.#poll(),2000);}
  async #poll(){
    const id=this.#job?.id;if(!id)return;
    try{const job=await this.#bridge.request('update_job_status',{job_id:id});if(this.#job?.id!==id)return;this.#job=job;if(job.state==='running')this.#schedule();else{await this.refresh();if(job.error&&job.state!=='cancelled')this.#notify(job.error.message,true);if(job.state==='complete')this.#notify(text('done'));}}
    catch(error){if(this.#job?.id!==id)return;this.#notify(error.message,true);clearTimeout(this.#timer);this.#job=null;await this.refresh();}
  }
}
