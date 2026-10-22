import {currentLocale,t} from './i18n.js';
const el=(tag,className,text)=>{const node=document.createElement(tag);if(className)node.className=className;if(text!==undefined)node.textContent=text;return node;};
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
export class BrowserDashboard{
  #bridge;#root;#toast;#available=false;#active=false;#busy=false;#generation=0;#tabs=[];#cursor=null;#total=0;#truncated=false;#nodes={};
  constructor(bridge,root,toast){
    this.#bridge=bridge;this.#root=root;this.#toast=toast;
    document.addEventListener('omniplay_locale_changed',()=>{if(this.#available)this.#render();});
    document.addEventListener('visibilitychange',()=>{if(!document.hidden&&this.#active)void this.refresh();});
    window.addEventListener('focus',()=>{if(this.#active)void this.refresh();});
    window.addEventListener('pagehide',()=>{this.#tabs=[];this.#root.replaceChildren();});
  }
  async initialize(){
    if(location.protocol!=='chrome-extension:')return false;
    try{const info=await this.#bridge.request('browser_info',undefined,10000);this.#available=info?.available===true;}catch{this.#available=false;}
    if(this.#available){this.#render();if(this.#active)await this.refresh();}return this.#available;
  }
  setActive(value){this.#active=Boolean(value);this.#generation++;if(!value){this.#tabs=[];this.#cursor=null;this.#total=0;if(this.#available)this.#renderTabs();}else if(this.#available)void this.refresh();}
  async #operation(button,action){if(button)button.disabled=true;try{return await action();}catch(error){this.#error(error);}finally{if(button?.isConnected)button.disabled=false;}}
  #error(error){const message=error?.code==='invalid_address'?text('Nhập địa chỉ web hợp lệ. Mặc định dùng HTTPS.','Enter a valid web address. HTTPS is the default.'):error?.code==='stale_cursor'?text('Danh sách tab đã hết hạn. Làm mới để tiếp tục.','This tab list expired. Refresh to continue.'):text('Chưa hoàn tất thao tác trình duyệt. Làm mới trước khi thử lại.','The browser operation could not be completed. Refresh before trying again.');this.#toast(message,true);}
  #button(label,action,className='button button-secondary'){const button=el('button',className,label);button.type='button';button.addEventListener('click',()=>void this.#operation(button,action));return button;}
  #render(){
    const oldUrl=this.#nodes.url?.value??'',oldSearch=this.#nodes.search?.value??'';this.#root.replaceChildren();
    const heading=el('div','page-heading'),headingText=el('div');headingText.append(el('p','eyebrow','YOUR BROWSER'),el('h1','',t('browser')),el('p','muted',text('Mở web, tìm kiếm và quản lý các tab trong cửa sổ này.','Open websites, search and manage tabs in this window.')));heading.append(headingText);this.#root.append(heading);
    const forms=el('div','browser-launch-grid');
    const addressForm=el('form','panel browser-launch'),addressLabel=el('label','field-label'),url=el('input');url.type='text';url.inputMode='url';url.maxLength=2048;url.autocomplete='off';url.autocapitalize='none';url.spellcheck=false;url.placeholder='example.com';url.value=oldUrl;
    addressLabel.append(el('span','',text('Địa chỉ web','Web address')),url);const addressButton=el('button','button button-primary',text('Mở trong tab mới','Open in a new tab'));addressButton.type='submit';addressForm.append(addressLabel,addressButton,el('p','muted small',text('Mặc định dùng HTTPS. Bạn có thể nhập rõ http:// khi cần.','HTTPS is the default. Enter http:// explicitly when needed.')));
    addressForm.addEventListener('submit',event=>{event.preventDefault();if(!url.value.trim())return;void this.#operation(addressButton,async()=>{await this.#bridge.request('browser_open_url',{url:url.value.trim()},10000);url.value='';});});
    const searchForm=el('form','panel browser-launch'),searchLabel=el('label','field-label'),search=el('input');search.type='search';search.maxLength=512;search.autocomplete='off';search.value=oldSearch;search.placeholder=text('Tìm trên web…','Search the web…');searchLabel.append(el('span','',text('Tìm trên web','Search the web')),search);const searchButton=el('button','button button-secondary',text('Tìm kiếm','Search'));searchButton.type='submit';searchForm.append(searchLabel,searchButton,el('p','muted small',text('Dùng công cụ tìm kiếm bạn đã chọn trong trình duyệt.','Uses the search provider selected in your browser.')));
    searchForm.addEventListener('submit',event=>{event.preventDefault();if(!search.value.trim())return;void this.#operation(searchButton,async()=>{await this.#bridge.request('browser_search',{text:search.value.trim()},10000);search.value='';});});forms.append(addressForm,searchForm);this.#root.append(forms);
    const shortcuts=el('section','panel browser-shortcuts');shortcuts.append(el('h2','',text('Lối tắt trình duyệt','Browser shortcuts')));const shortcutGrid=el('div','browser-shortcut-grid');
    const labels={bookmarks:text('Dấu trang','Bookmarks'),history:text('Lịch sử','History'),downloads:text('Tải xuống','Downloads'),performance:text('Bộ nhớ và hiệu suất','Memory and performance'),permissions:text('Quyền trang web','Site permissions'),privacy:text('Quyền riêng tư','Privacy'),passwords:text('Mật khẩu','Passwords'),profiles:text('Hồ sơ duyệt web','Browser profiles'),search:text('Công cụ tìm kiếm','Search provider')};
    for(const[page,label]of Object.entries(labels))shortcutGrid.append(this.#button(label,()=>this.#bridge.request('browser_open_internal',{page},10000)));
    shortcuts.append(shortcutGrid,el('p','muted small',text('Các mục này mở trang cài đặt gốc của trình duyệt. OmniPlay không đọc mật khẩu, nội dung trang hoặc lịch sử duyệt web để tạo gợi ý game.','These open the browser’s built-in pages. OmniPlay does not read passwords, page content or browsing history to recommend games.')));this.#root.append(shortcuts);
    const section=el('section','panel browser-tabs'),bar=el('div','browser-tabs-heading');bar.append(el('h2','',text('Tab trong cửa sổ này','Tabs in this window')));const refresh=this.#button(text('Làm mới','Refresh'),()=>this.refresh());bar.append(refresh);const status=el('p','muted small');status.setAttribute('role','status');const list=el('ul','browser-tab-list');list.setAttribute('aria-label',text('Danh sách tab','Browser tabs'));const more=this.#button(text('Xem thêm tab','More tabs'),()=>this.refresh(false));
    section.append(bar,status,list,more,el('p','muted small',text('Danh sách chỉ hiển thị tên và nguồn trang. Trạng thái được cập nhật khi làm mới. Không tự đóng hoặc ngủ đông tab đang chơi.','Only tab titles and origins appear here. Refresh to update their status. Playing tabs are never automatically closed or discarded.')));this.#root.append(section);this.#nodes={url,search,status,list,more,refresh};this.#renderTabs();
  }
  #renderTabs(){
    const{list,status,more}=this.#nodes;if(!list)return;list.replaceChildren();status.textContent=this.#busy?text('Đang lấy danh sách…','Loading tabs…'):this.#tabs.length+' / '+this.#total+' '+text('tab','tabs')+(this.#truncated?text(' · Hiển thị tối đa 2.000 tab.',' · Showing up to 2,000 tabs.'):'');
    for(const tab of this.#tabs){
      const row=el('li','browser-tab-row'),main=el('div','browser-tab-description'),title=el('strong','',tab.title||text('Tab chưa có tên','Untitled tab'));title.dir='auto';const origin=el('span','browser-tab-origin',tab.origin);origin.dir='ltr';main.append(title,origin);const states=[];
      if(tab.active)states.push(text('Đang mở','Active'));if(tab.pinned)states.push(text('Đã ghim','Pinned'));if(tab.audible)states.push(text('Có âm thanh','Playing audio'));if(tab.muted)states.push(text('Đã tắt tiếng','Muted'));if(tab.discarded)states.push(text('Đã giải phóng bộ nhớ','Discarded'));if(tab.loading)states.push(text('Đang tải','Loading'));if(states.length)main.append(el('small','muted',states.join(' · ')));
      const actions=el('div','browser-tab-actions'),activate=this.#button(text('Chuyển tới','Switch to'),()=>this.#bridge.request('browser_activate_tab',{tab_id:tab.id},10000));activate.disabled=tab.active;const pin=this.#button(tab.pinned?text('Bỏ ghim','Unpin'):text('Ghim','Pin'),async()=>{await this.#bridge.request('browser_pin_tab',{tab_id:tab.id,pinned:!tab.pinned},10000);await this.refresh();});activate.setAttribute('aria-label',text('Chuyển tới tab: ','Switch to tab: ')+(tab.title||tab.origin));pin.setAttribute('aria-label',(tab.pinned?text('Bỏ ghim tab: ','Unpin tab: '):text('Ghim tab: ','Pin tab: '))+(tab.title||tab.origin));actions.append(activate,pin);row.append(main,actions);list.append(row);
    }
    more.hidden=!this.#cursor;more.disabled=this.#busy;
  }
  async refresh(reset=true){
    if(!this.#available||!this.#active||this.#busy)return;const generation=this.#generation;this.#busy=true;this.#renderTabs();
    try{const page=await this.#bridge.request('browser_tabs_page',{after:reset?null:this.#cursor},10000);if(!this.#active||generation!==this.#generation)return;
      if(reset)this.#tabs=[];const known=new Set(this.#tabs.map(tab=>tab.id));this.#tabs.push(...page.tabs.filter(tab=>!known.has(tab.id)));this.#cursor=page.next_cursor;this.#total=page.total;this.#truncated=page.truncated;
    }catch(error){if(this.#active)this.#error(error);}finally{this.#busy=false;this.#renderTabs();if(this.#active&&generation!==this.#generation)void this.refresh();}
  }
}
