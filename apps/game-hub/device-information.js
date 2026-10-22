import{currentLocale}from'./i18n.js';
const text=(vi,en)=>currentLocale()==='vi'?vi:en;
const node=(tag,className,value)=>{const element=document.createElement(tag);if(className)element.className=className;if(value!==undefined)element.textContent=value;return element;};
const unknown=()=>text('Chưa xác định','Unknown');
function amount(value){return Number.isSafeInteger(value)&&value>=0?(value/1024**3).toFixed(2)+' GiB':unknown();}
function bool(value){return typeof value==='boolean'?(value?text('Có','Yes'):text('Không','No')):unknown();}
function safe(value){return typeof value==='string'&&value.length<=256?value:unknown();}
export class DeviceInformationPanel{
  #root;#bridge;#report=null;#busy=false;#attempted=false;#error=false;
  constructor(root,bridge){this.#root=root;this.#bridge=bridge;document.addEventListener('omniplay_locale_changed',()=>this.#render());this.#render();}
  async refresh(force=false){
    if(this.#busy||this.#attempted&&!force)return;
    if(document.querySelector('meta[name="omniplay-platform"]')?.content==='web'){this.#root.hidden=true;return;}
    this.#attempted=true;this.#busy=true;this.#error=false;this.#render();
    try{const report=await this.#bridge.request('device_snapshot',undefined,10000);if(report?.schema_version!==1||!report.power||!Array.isArray(report.graphics_adapters)||report.graphics_adapters.length>8||report.benchmark_evidence!==false)throw new Error('Invalid inventory response');this.#report=report;}
    catch{this.#error=true;}
    finally{this.#busy=false;this.#render();}
  }
  #render(){
    const panel=node('section','panel native-device-information'),heading=node('div','browser-tabs-heading');heading.append(node('h2','',text('Thông tin hệ thống cục bộ','Local system information')));const refresh=node('button','button button-secondary',text('Làm mới','Refresh'));refresh.type='button';refresh.disabled=this.#busy;refresh.addEventListener('click',()=>void this.refresh(true));heading.append(refresh);panel.append(heading);
    const status=node('p','muted small');status.setAttribute('role','status');status.textContent=this.#busy?text('Đang đọc thông tin hệ thống…','Collecting system information…'):this.#error?text('Chưa lấy được thông tin. Bạn có thể làm mới để thử lại.','Information is unavailable. Refresh to try again.'):!this.#report?text('Mở mục Thiết bị hoặc làm mới để đọc thông tin.','Open Devices or refresh to collect information.'):text('Ảnh chụp thông tin, không phải kết quả benchmark.','An inventory snapshot, not benchmark evidence.');panel.append(status);
    const report=this.#report;
    if(report){
      const list=node('dl','definition-list');const row=(label,value)=>{const item=node('div');item.append(node('dt','',label),node('dd','',value));list.append(item);};
      row(text('Nguồn dữ liệu','Source'),safe(report.source));row(text('Hệ điều hành','Operating system'),safe(report.operating_system));row(text('Kiến trúc tiến trình','Process architecture'),safe(report.process_architecture));row(text('Phiên bản kernel','Kernel version'),safe(report.kernel_version));row(text('Mức song song khả dụng','Available parallelism'),Number.isSafeInteger(report.available_parallelism)?String(report.available_parallelism):unknown());
      row(text('Phiên bản nền tảng','Platform version'),safe(report.platform_version));row(text('Nguồn thông tin nền tảng','Platform data source'),safe(report.platform_source));row(text('Trạng thái nhiệt do hệ điều hành báo','OS-reported thermal state'),safe(report.thermal_state));row(text('RAM hệ thống','System memory'),amount(report.total_memory_bytes));row(text('RAM khả dụng lúc đo','Available memory at collection'),amount(report.available_memory_bytes));row(text('Có pin hệ thống','System battery present'),bool(report.power.battery_present));row(text('Đang nối nguồn','External power connected'),bool(report.power.ac_connected));row(text('Pin đang sạc','Battery charging'),bool(report.power.charging));row(text('Mức pin','Battery level'),Number.isInteger(report.power.battery_percent)&&report.power.battery_percent>=0&&report.power.battery_percent<=100?report.power.battery_percent+'%':unknown());row(text('Chế độ tiết kiệm pin','Battery saver'),bool(report.power.battery_saver));
      const at=new Date(report.sampled_at_ms);row(text('Thời điểm thu thập','Collected at'),Number.isFinite(at.getTime())?at.toLocaleString(currentLocale()):unknown());if(Number.isSafeInteger(report.platform_sampled_at_ms)){const at=new Date(report.platform_sampled_at_ms);row(text('Thời điểm đọc nền tảng','Platform data collected at'),Number.isFinite(at.getTime())?at.toLocaleString(currentLocale()):unknown());}panel.append(list);
      for(const[index,gpu]of report.graphics_adapters.entries()){
        const section=node('section','device-graphics-adapter');section.append(node('h3','',text('GPU được hệ điều hành liệt kê ','OS-enumerated GPU ')+(index+1)),node('p','',safe(gpu.name)));const detail=node('dl','definition-list');
        const item=(label,value)=>{const row=node('div');row.append(node('dt','',label),node('dd','',value));detail.append(row);};
        item(text('Mã vendor / thiết bị','Vendor / device ID'),[gpu.vendor_id,gpu.device_id].map(value=>Number.isInteger(value)&&value>=0&&value<=0xffffffff?'0x'+value.toString(16):unknown()).join(' / '));item(text('Bộ nhớ chuyên dụng','Dedicated memory'),amount(gpu.dedicated_memory_bytes));item(text('Bộ nhớ chia sẻ','Shared memory'),amount(gpu.shared_memory_bytes));item(text('Adapter phần mềm','Software adapter'),bool(gpu.software));item(text('Phiên bản driver (giá trị gốc)','Driver version (raw value)'),safe(gpu.driver_version));item(text('Nguồn phiên bản driver','Driver version source'),safe(gpu.driver_version_source));section.append(detail);panel.append(section);
      }
      if(!report.graphics_adapters.length)panel.append(node('p','muted small',text('Chưa có dữ liệu GPU từ adapter của nền tảng này.','No GPU inventory is available through this platform adapter.')));
    }
    panel.append(node('p','muted small',text('Chỉ đọc khi bạn mở mục này hoặc làm mới. Thông tin không tự gửi lên mạng hay gói chẩn đoán. GPU được liệt kê chưa chắc là GPU đang render game; không suy ra FPS, nhiệt độ hoặc throttling từ cấu hình.','Collected when you open this panel or refresh. This information is not automatically sent to the network or diagnostics. Listed GPUs may differ from the game’s rendering adapter; inventory does not establish FPS, temperature or throttling.')));this.#root.replaceChildren(panel);
  }
}
