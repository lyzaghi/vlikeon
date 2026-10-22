import{currentLocale}from'./i18n.js';
const vi=()=>currentLocale()==='vi';
function node(tag,className,text){const item=document.createElement(tag);item.className=className;if(text!==undefined)item.textContent=text;return item;}
const roles=[['snes','snes_spc_ipl','SNES · SPC IPL · 64 B'],['mega_drive','mega_drive_svp','Mega Drive · SVP · 2 KiB'],['ps1','ps1_bios_jp','PlayStation · BIOS Japan · 512 KiB'],['ps1','ps1_bios','PlayStation · BIOS USA · 512 KiB'],['ps1','ps1_bios_eu','PlayStation · BIOS Europe · 512 KiB'],['nds','nds_bios7','Nintendo DS · BIOS7 · 16 KiB'],['nds','nds_bios9','Nintendo DS · BIOS9 · 4 KiB'],['nds','nds_firmware','Nintendo DS · firmware']];
export class FirmwareSettings {
  #bridge;#container;#notify;#busy=false;#revision=0;#role='snes:snes_spc_ipl';
  constructor(bridge,container,notify){this.#bridge=bridge;this.#container=container;this.#notify=notify;}
  #lock(){for(const item of this.#container.querySelectorAll('button,select'))item.disabled=this.#busy;}
  #button(label,action){const button=node('button','button button-secondary',label);button.type='button';button.disabled=this.#busy;button.addEventListener('click',async()=>{if(this.#busy)return;this.#busy=true;this.#lock();try{await action();}catch(error){if(error.code!=='cancelled')this.#notify(error.message,true);}finally{this.#busy=false;this.#lock();}});return button;}
  async refresh(){
    const revision=++this.#revision;
    try{
      const assets=await this.#bridge.request('firmware_list');if(revision!==this.#revision)return;
      if(!Array.isArray(assets)||assets.length>128||assets.some(asset=>!asset||typeof asset.content_hash!=='string'||asset.content_hash.length!==64||!/^[a-f0-9]+$/.test(asset.content_hash)||!Number.isSafeInteger(asset.byte_length)||asset.byte_length<=0||asset.byte_length>524288||!Array.isArray(asset.selected_roles)||asset.selected_roles.length>8||new Set(asset.selected_roles).size!==asset.selected_roles.length||asset.selected_roles.some(role=>!roles.some(([system,id])=>role===`${system}:${id}`)))||new Set(assets.map(asset=>asset.content_hash)).size!==assets.length)throw new Error(vi()?'Danh sách firmware chưa hợp lệ.':'The firmware inventory is invalid.');
      const panel=node('section','panel sync-panel');panel.append(node('h2','',vi()?'Firmware của bạn':'Your firmware'),node('p','muted small',vi()?'Chỉ nhập tệp bạn có quyền sử dụng. Ứng dụng không kèm BIOS của nhà sản xuất. Firmware được giữ cục bộ và không đồng bộ lên dịch vụ. Kiểm tra kích thước và checksum không xác nhận nguồn gốc hoặc khả năng tương thích.':'Import only files you are authorized to use. No manufacturer BIOS is included. Firmware stays local and is never synchronized. Size and checksum checks do not establish provenance or compatibility.'));
      const picker=node('select');picker.setAttribute('aria-label',vi()?'Vai trò firmware':'Firmware role');for(const[system,role,label]of roles){const option=node('option','',label);option.value=`${system}:${role}`;picker.append(option);}
      picker.value=this.#role;picker.addEventListener('change',()=>{this.#role=picker.value;});
      panel.append(picker,this.#button(vi()?'Nhập từ thiết bị':'Import from device',async()=>{const[system,role]=picker.value.split(':');await this.#bridge.request('firmware_import_selected',{system,role},120000);await this.refresh();}));
      panel.append(node('p','muted small',`${assets.length}/128 · ${(assets.reduce((total,asset)=>total+asset.byte_length,0)/1024**2).toFixed(2)} MiB`));
      for(const asset of assets){const row=node('div','save-item'),details=node('div');details.append(node('p','small',`${asset.byte_length} B · ${asset.content_hash.slice(0,16)}…`),node('small','muted',asset.selected_roles.map(role=>roles.find(([system,id])=>role===`${system}:${id}`)[2]).join(' · ')||(vi()?'Chưa chọn cho runtime':'Not selected for a runtime')));
        const actions=node('div','detail-actions');actions.append(this.#button(vi()?'Chọn cho vai trò trên':'Use for selected role',async()=>{const[system,role]=picker.value.split(':');await this.#bridge.request('firmware_select',{system,role,content_hash:asset.content_hash});await this.refresh();}),this.#button(vi()?'Xóa':'Delete',async()=>{if(!window.confirm(vi()?'Xóa tệp firmware này và các lựa chọn liên quan? Snapshot phụ thuộc nó có thể cần nhập lại tệp để sử dụng.':'Delete this firmware and its selections? Snapshots that depend on it may require importing it again.'))return;await this.#bridge.request('firmware_forget',{content_hash:asset.content_hash});await this.refresh();}));row.append(details,actions);panel.append(row);}
      this.#container.replaceChildren(panel);
      this.#lock();
    }catch(error){if(revision===this.#revision)this.#container.replaceChildren(node('p','muted small',error.message));}
  }
}
