import{currentLocale,exportMessage}from'./i18n.js';
const vi=()=>currentLocale()==='vi';
function node(tag,className,text){const element=document.createElement(tag);element.className=className;if(text!==undefined)element.textContent=text;return element;}

export async function mountPerformanceHistory(container,bridge,game,notify){
  try{
    const runs=await bridge.request('performance_run_list',{game_id:game.id});container.replaceChildren(node('h3','',vi()?'Hiệu năng đã đo':'Measured performance'));
    if(!runs.length){container.append(node('p','muted small',vi()?'Chưa có phép đo hoàn chỉnh. Khi đang chơi, bạn có thể chủ động đo sau 10 giây làm nóng và 2 phút chơi liên tục.':'No completed measurement. During gameplay you can choose a 10-second warm-up followed by 2 minutes of uninterrupted play.'));return;}
    container.append(node('p','muted small',vi()?'Số đo từ phiên chơi trên thiết bị. So sánh giữa các phiên chưa phải phép đo A/B có kiểm soát. Tối đa 20 phép đo được giữ cục bộ.':'Measurements come from local gameplay. Cross-session comparisons are not controlled hardware A/B measurements. Up to 20 runs are retained locally.'));
    for(const run of runs){
      const native=run.measurement_source==='native_frontend_steady_clock',origin=native?(vi()?'Cửa sổ game':'Game window'):(vi()?'Trình duyệt':'Browser');
      const row=node('div','save-item'),description=node('div');description.append(node('p','small',`${new Date(run.created_at_ms).toLocaleString(currentLocale())} · ${origin} · ${run.preset}`),node('small','muted',`p95 ${run.frame_time_p95_ms.toFixed(2)} ms · ${run.frames_per_second.toFixed(1)} ${native?'core ticks/s':'emulated FPS'} · ${run.presented_frames_per_second.toFixed(1)} presented FPS`));
      const actions=node('div','detail-actions');
      const button=(title,action)=>{const item=node('button','button button-secondary',title);item.type='button';item.addEventListener('click',async()=>{item.disabled=true;try{await action();}catch(error){notify(error.message,true);}finally{item.disabled=false;}});return item;};
      actions.append(button(vi()?'Xuất số đo':'Export measurements',async()=>{const grant=await bridge.request('file_select_export',{suggested_name:`omniplay-performance-${run.id}.json`},120000);const result=await bridge.request('performance_run_export',{run_id:run.id,destination_grant_id:grant.grant_id},120000);notify(exportMessage(result));}),button(vi()?'Xóa':'Delete',async()=>{await bridge.request('performance_run_delete',{run_id:run.id});await mountPerformanceHistory(container,bridge,game,notify);}));
      row.append(description,actions);container.append(row);
    }
    if(runs.length>=2){
      const label=node('p','small',vi()?'So sánh hai phiên gần nhất':'Compare the two latest sessions');const button=node('button','button button-secondary',vi()?'So sánh':'Compare');button.type='button';
      button.addEventListener('click',async()=>{button.disabled=true;try{const comparison=await bridge.request('performance_run_compare',{baseline_id:runs[1].id,candidate_id:runs[0].id});label.textContent=`p95 ${comparison.frame_time_p95_change_percent>=0?'+':''}${comparison.frame_time_p95_change_percent.toFixed(1)}% · audio ${comparison.audio_underrun_change>=0?'+':''}${comparison.audio_underrun_change} · ${vi()?'giá trị p95 thấp hơn là tốt hơn; điều kiện có thể khác nhau':'lower p95 is better; conditions may differ'}`;}catch(error){notify(error.message,true);}finally{button.disabled=false;}});container.append(label,button);
    }
  }catch(error){container.replaceChildren(node('p','muted small',error.message));}
}
