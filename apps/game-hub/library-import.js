import{currentLocale}from'./i18n.js';
const copy={
  vi:{title:'Nhập bộ đĩa',description:'Chọn CUE hoặc M3U trong thư mục đã cho phép. Chỉ những tệp được bộ đĩa tham chiếu mới được sao chép. Thư viện và dữ liệu nguồn được giữ riêng.',name:'Tên game',entry:'Tệp mở bộ đĩa',system:'Hệ máy',encoding:'Mã hóa văn bản CUE/M3U',start:'Thêm vào thư viện',close:'Đóng',progress:'Đang nhập game',preparing:'Đang đọc danh sách tệp…',cancel:'Dừng nhập',complete:'Đã thêm game vào thư viện.',cancelled:'Đã dừng nhập game.',failed:'Không thể hoàn tất nhập game.',pending:'Đang dừng sau thao tác đọc hiện tại…',committing:'Đang hoàn tất thêm vào thư viện. Chờ thao tác kết thúc.'},
  en:{title:'Import a disc set',description:'Choose a CUE or M3U entry within the authorized folder. Only the referenced disc files are copied. The library and source files remain separate.',name:'Game title',entry:'Disc entry point',system:'System',encoding:'CUE/M3U text encoding',start:'Add to library',close:'Close',progress:'Importing game',preparing:'Reading the asset list…',cancel:'Stop import',complete:'Game added to your library.',cancelled:'Game import cancelled.',failed:'Game import could not finish.',pending:'Stopping after the current read operation…',committing:'Finishing the library import. Wait for the operation to complete.'}
};
const text=key=>(copy[currentLocale()]||copy.en)[key];
function node(tag,className,value){const item=document.createElement(tag);item.className=className;if(value!==undefined)item.textContent=value;return item;}
function field(label,input){const wrapper=node('label','field-label');wrapper.append(node('span','',label),input);return wrapper;}

export class LibraryImporter {
  #bridge;#notify;#changed;#job=null;#timer=0;#panel=null;#progress=null;#label=null;#cancel=null;#revision=0;
  constructor(bridge,notify,changed){this.#bridge=bridge;this.#notify=notify;this.#changed=changed;}
  async startFile(grant_id,title,system,format_hint=null){this.#track(await this.#bridge.request('library_import_start',{grant_id,title,system,format_hint}));}
  async chooseDisc(){
    const selection=await this.#bridge.request('disc_select_folder',undefined,120000);let used=false;
    const dialog=node('dialog','sync-dialog'),form=node('form');form.append(node('h2','',text('title')),node('p','muted small',text('description')));
    const title=node('input');title.name='title';title.value=selection.suggested_title;title.maxLength=150;title.required=true;
    const entry=node('select');entry.name='entry';for(const filename of selection.entries){const option=node('option','',filename);option.value=filename;entry.append(option);}
    const system=node('select');system.name='system';for(const[value,label]of[['ps1','PlayStation'],['psp','PSP']]){const option=node('option','',label);option.value=value;system.append(option);}
    const encoding=node('select');encoding.name='encoding';for(const[value,label]of[['auto','UTF-8 / BOM'],['windows1252','Windows-1252'],['shift_jis','Shift JIS'],['gbk','GBK'],['utf16_le','UTF-16 LE'],['utf16_be','UTF-16 BE']]){const option=node('option','',label);option.value=value;encoding.append(option);}
    const actions=node('div','detail-actions'),submit=node('button','button button-primary',text('start')),close=node('button','button button-secondary',text('close'));submit.type='submit';close.type='button';close.addEventListener('click',()=>dialog.close());actions.append(submit,close);
    form.append(field(text('name'),title),field(text('entry'),entry),field(text('system'),system),field(text('encoding'),encoding),actions);
    form.addEventListener('submit',async event=>{event.preventDefault();submit.disabled=true;try{const job=await this.#bridge.request('library_import_bundle_start',{grant_id:selection.grant_id,entry_name:entry.value,title:title.value.trim(),system:system.value,encoding:encoding.value});used=true;dialog.close();this.#track(job);}catch(error){this.#notify(error.message,true);}finally{submit.disabled=false;}});
    dialog.addEventListener('close',()=>{if(!used)void this.#bridge.request('permission_revoke',{grant_id:selection.grant_id}).catch(()=>{});dialog.remove();});dialog.append(form);document.body.append(dialog);dialog.showModal();
  }
  #track(job){
    const revision=++this.#revision;clearTimeout(this.#timer);this.#job=job;this.#panel?.remove();this.#panel=node('section','import-progress-panel');this.#panel.setAttribute('aria-label',text('progress'));
    this.#label=node('p','small',text('preparing'));this.#label.setAttribute('role','status');this.#progress=node('progress');this.#progress.max=1;this.#progress.setAttribute('aria-label',text('progress'));
    this.#cancel=node('button','button button-secondary',text('cancel'));this.#cancel.type='button';this.#cancel.addEventListener('click',async()=>{this.#cancel.disabled=true;try{const result=await this.#bridge.request('library_import_cancel',{job_id:job.id});if(revision!==this.#revision)return;this.#label.textContent=text(result.cancel_accepted===false?'committing':'pending');}catch(error){if(revision!==this.#revision)return;this.#notify(error.message,true);this.#cancel.disabled=false;}});
    this.#panel.append(this.#label,this.#progress,this.#cancel);document.body.append(this.#panel);this.#render();this.#schedule(revision);
  }
  #render(){const p=this.#job.progress;if(p.phase==='committing'){this.#cancel.disabled=true;this.#label.textContent=text('committing');return;}if(p.total_bytes>0){this.#progress.value=Math.min(1,p.copied_bytes/p.total_bytes);this.#label.textContent=`${text('progress')} · ${Math.floor(this.#progress.value*100)}% · ${p.asset_index}/${p.asset_count}`;}else{this.#progress.removeAttribute('value');this.#label.textContent=p.phase==='copying'?`${text('progress')} · ${(p.copied_bytes/(1024*1024)).toFixed(1)} MiB · ${p.asset_index}/${p.asset_count}`:text('preparing');}}
  #schedule(revision){this.#timer=setTimeout(()=>void this.#poll(revision),750);}
  async #poll(revision){
    if(revision!==this.#revision)return;const id=this.#job.id;
    try{
      const result=await this.#bridge.request('library_import_status',{job_id:id});if(revision!==this.#revision)return;this.#job=result;this.#render();if(result.state==='running'){this.#schedule(revision);return;}
      const panel=this.#panel;this.#cancel.remove();this.#label.textContent=text(result.state==='complete'?'complete':result.state==='cancelled'?'cancelled':'failed');
      if(result.state==='complete'){this.#notify(text('complete'));await this.#changed(result.game);}
      else if(result.state==='failed')this.#notify(result.error?.message||text('failed'),true);
      setTimeout(()=>panel.remove(),6000);
    }catch(error){if(revision!==this.#revision)return;this.#notify(error.message,true);this.#cancel.disabled=true;this.#label.textContent=error.message;}
  }
}
