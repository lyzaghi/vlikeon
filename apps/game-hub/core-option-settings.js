import{currentLocale}from'./i18n.js';
import{coreOptionRuntime,defaultCoreOptions,validateCoreOptions,validateCoreOptionPolicy,webCoreOptionPolicy,resolveCoreOptions}from'./runtime/core-options.js';
const mounts=new WeakMap();
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
function node(tag,className,value){const item=document.createElement(tag);if(className)item.className=className;if(value!==undefined)item.textContent=value;return item;}
const localized=value=>value[currentLocale()==='vi'?'label_vi':'label_en'];
function name(id){return id.replace(/-(native|web)$/,'').split('-').map(part=>({psx:'PSX',ps1:'PS1',mgba:'mGBA',ppsspp:'PPSSPP',ds:'DS',md:'MD',gb:'GB',gbc:'GBC',n64:'N64'}[part]??part.charAt(0).toUpperCase()+part.slice(1))).join(' ');}

export async function mountCoreOptionSettings(container,bridge,game,notify){
  const token={};mounts.set(container,token);const alive=()=>container.isConnected&&mounts.get(container)===token;
  container.replaceChildren(node('p','muted small',text('Đang lấy tuỳ chọn cho game…','Loading game options…')));
  try{
    const[response,native,stored]=await Promise.all([fetch(new URL('./cores/catalog.json',import.meta.url),{credentials:'same-origin',redirect:'error'}),bridge.request('runtime_list'),bridge.request('game_core_options_list',{game_id:game.id})]);
    if(!alive())return;if(!response.ok)throw new Error(text('Chưa đọc được danh sách trình giả lập.','The installed emulator list could not be read.'));
    const raw=await response.text();if(new TextEncoder().encode(raw).length>1024*1024)throw new Error('The installed emulator list exceeds its limit.');const catalog=JSON.parse(raw);
    if(catalog?.schema_version!==1||!Array.isArray(catalog.cores)||catalog.cores.length>32||!Array.isArray(native)||native.length>32||!Array.isArray(stored)||stored.length>32)throw new Error('The installed emulator list is invalid.');
    const runtimes=[],ids=new Set();
    for(const core of catalog.cores){if(!core?.systems?.includes(game.system)||core.distribution_approved!==true||core.security_revoked===true)continue;coreOptionRuntime(core.id);if(ids.has(core.id))throw new Error('Installed emulator identities are duplicated.');ids.add(core.id);runtimes.push({id:core.id,title:name(core.id)+' · '+text('Trong trình duyệt','In browser'),policy:webCoreOptionPolicy(core)});}
    for(const core of native){if(!core?.systems?.includes(game.system))continue;coreOptionRuntime(core.id);if(ids.has(core.id))throw new Error('Installed emulator identities are duplicated.');ids.add(core.id);runtimes.push({id:core.id,title:name(core.id)+' · '+text('Cửa sổ riêng','Separate window'),policy:null});}
    const savedIds=new Set();for(const id of stored){coreOptionRuntime(id);if(savedIds.has(id))throw new Error('Saved emulator identities are duplicated.');savedIds.add(id);if(!ids.has(id)){ids.add(id);runtimes.push({id,title:name(id)+' · '+text('Chưa khả dụng','Unavailable'),policy:null,unavailable:true});}}
    if(!alive())return;
    const title=node('h3','',text('Tuỳ chỉnh trình giả lập cho game','Emulator options for this game'));
    const note=node('p','muted small',text('Áp dụng khi mở game lần tới. Snapshot có cấu hình khác được giữ lại nhưng có thể không tải được. Khôi phục mặc định sẽ xoá lựa chọn đã lưu của trình giả lập đang chọn.','Applies on the next launch. Snapshots with other settings remain in history but may be incompatible. Reset removes saved choices for the selected emulator.'));
    if(!runtimes.length){container.replaceChildren(title,node('p','muted small',text('Tuỳ chọn xuất hiện sau khi cài trình giả lập phù hợp.','Options appear when a compatible emulator is installed.')));return;}
    const form=node('form','game-settings-form'),select=node('select'),label=node('label','field-label'),fields=node('div','core-option-fields'),status=node('p','muted small');status.setAttribute('role','status');
    select.setAttribute('aria-label',text('Trình giả lập','Emulator'));for(const runtime of runtimes){const option=node('option','',runtime.title);option.value=runtime.id;select.append(option);}label.append(node('span','',text('Trình giả lập','Emulator')),select);
    const actions=node('div','detail-actions'),save=node('button','button button-primary',text('Lưu tuỳ chỉnh','Save options')),reset=node('button','button button-secondary',text('Khôi phục mặc định đã lưu','Reset saved options'));save.type='submit';reset.type='button';actions.append(save,reset);form.append(label,fields,actions,status);container.replaceChildren(title,note,form);
    let revision=0,policy=null,profile=null,busy=false;const drafts=new Map();
    function lock(value){busy=value;select.disabled=value;save.disabled=value||!profile||!policy?.definitions.length;reset.disabled=value;for(const field of fields.querySelectorAll('select'))field.disabled=value;}
    function paint(){
      fields.replaceChildren();const effective=resolveCoreOptions(policy,profile);
      if(!policy.definitions.length)fields.append(node('p','muted small',text('Trình giả lập này giữ cấu hình cố định.','This emulator uses fixed settings.')));
      for(const definition of policy.definitions){
        const row=node('label','field-label'),choice=node('select'),description=node('p','muted small',definition[currentLocale()==='vi'?'description_vi':'description_en']);
        description.id='core-option-'+crypto.randomUUID();choice.setAttribute('aria-describedby',description.id);
        for(const value of definition.choices){const item=node('option','',localized(value));item.value=value.value;choice.append(item);}choice.value=effective[definition.key];
        choice.addEventListener('change',()=>{if(busy||!profile)return;if(choice.value===definition.default_value)delete profile.values[definition.key];else profile.values[definition.key]=choice.value;status.textContent=text('Bản nháp đã đổi. Nhấn Lưu để dùng ở lần chơi tới.','Draft changed. Save to use it on the next launch.');});row.append(node('span','',localized(definition)),choice,description);fields.append(row);
      }lock(false);
    }
    async function load(){
      const current=++revision,runtime=runtimes.find(item=>item.id===select.value);policy=null;profile=null;lock(true);fields.replaceChildren();status.textContent=text('Đang lấy cấu hình đã lưu…','Loading saved choices…');
      if(runtime.unavailable){fields.append(node('p','muted small',text('Trình giả lập này hiện chưa khả dụng cho game. Có thể xoá lựa chọn đã lưu để giải phóng chỗ cấu hình.','This emulator is currently unavailable for the game. Reset its saved choices to free configuration storage.')));status.textContent='';lock(false);return;}
      try{
        const declared=runtime.policy??await bridge.request('runtime_options_get',{runtime_id:runtime.id});const nextPolicy=validateCoreOptionPolicy(declared);
        const saved=validateCoreOptions(drafts.has(runtime.id)?drafts.get(runtime.id):await bridge.request('game_core_options_get',{game_id:game.id,runtime_id:runtime.id}));resolveCoreOptions(nextPolicy,saved);
        if(!alive()||current!==revision)return;policy=nextPolicy;profile=saved;drafts.set(runtime.id,profile);paint();status.textContent=text('Chọn rồi lưu để áp dụng từ lần chơi tiếp theo.','Choose and save options for the next launch.');
      }catch(error){if(!alive()||current!==revision)return;status.textContent=error.message;lock(false);}
    }
    select.addEventListener('change',()=>void load());
    form.addEventListener('submit',event=>{event.preventDefault();if(busy||!profile||!policy)return;const runtimeId=select.value,value=validateCoreOptions(profile);lock(true);
      void bridge.request('game_core_options_set',{game_id:game.id,runtime_id:runtimeId,options:value}).then(saved=>{if(!alive())return;profile=validateCoreOptions(saved);drafts.set(runtimeId,profile);paint();status.textContent=text('Đã lưu tuỳ chỉnh cho lần chơi tới.','Options saved for the next launch.');}).catch(error=>{if(alive())notify(error.message,true);}).finally(()=>{if(alive())lock(false);});
    });
    reset.addEventListener('click',()=>{if(busy)return;const runtimeId=select.value;lock(true);
      void bridge.request('game_core_options_set',{game_id:game.id,runtime_id:runtimeId,options:defaultCoreOptions()}).then(async()=>{drafts.delete(runtimeId);if(alive())await load();}).catch(error=>{if(alive())notify(error.message,true);}).finally(()=>{if(alive())lock(false);});
    });
    await load();
  }catch(error){if(alive())container.replaceChildren(node('p','muted small',error.message));}
}
