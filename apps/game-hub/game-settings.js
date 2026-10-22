import{currentLocale}from'./i18n.js';
const labels={
  vi:{title:'Cấu hình game',preset:'Chế độ',accurate:'Đầy đủ khung hình',smooth:'Đệm âm thanh rộng hơn',battery:'Tiết kiệm · hiển thị tối đa 30 FPS',volume:'Âm lượng',latency:'Mức đệm âm thanh',save:'Lưu cấu hình',next:'Áp dụng từ phiên chơi tiếp theo. Tốc độ giả lập và âm thanh trong game được giữ nguyên.',saved:'Đã lưu cấu hình cho lần chơi tới.',assistant:'Gợi ý cục bộ',disabled:'Bật gợi ý cục bộ trong Cài đặt để sử dụng trợ lý. Nội dung không được gửi đến dịch vụ AI.',ask:'Xem gợi ý',accept:'Áp dụng ở lần chơi tới',dismiss:'Bỏ qua',audio:'Âm thanh bị ngắt',stutter:'Khung hình chưa đều',controls:'Kết nối điều khiển',saves:'Tiếp tục tiến trình',privacy:'Quyền riêng tư',batteryTopic:'Chơi khi dùng pin',applied:'Đã áp dụng gợi ý cho lần chơi tới.',local:'Quy tắc trên thiết bị · Không dùng mô hình bên ngoài'},
  en:{title:'Game settings',preset:'Playback mode',accurate:'Present every frame',smooth:'Larger audio buffer',battery:'Battery · present at most 30 FPS',volume:'Volume',latency:'Audio buffer level',save:'Save settings',next:'Applies to the next play session. Emulation and in-game audio speed stay unchanged.',saved:'Game settings saved for the next session.',assistant:'Local recommendations',disabled:'Enable local recommendations in Settings to use this assistant. No content is sent to an AI service.',ask:'Get advice',accept:'Apply to the next session',dismiss:'Dismiss',audio:'Audio interruptions',stutter:'Uneven frame times',controls:'Connect controls',saves:'Continue saved progress',privacy:'Privacy',batteryTopic:'Play on battery',applied:'Recommendation accepted for the next session.',local:'On-device rules · No external model'}
};
const text=key=>(labels[currentLocale()]||labels.en)[key];
function node(tag,className,value){const item=document.createElement(tag);item.className=className;if(value!==undefined)item.textContent=value;return item;}
function field(title,input){const label=node('label','field-label');label.append(node('span','',title),input);return label;}

export async function mountGameSettings(container,bridge,game,notify){
  try{
    const settings=await bridge.request('game_settings_get',{game_id:game.id}),global=await bridge.request('settings_get');
    const form=node('form','game-settings-form');form.append(node('h3','',text('title')));
    const preset=node('select');preset.name='preset';for(const key of['accurate','smooth','battery']){const option=node('option','',text(key));option.value=key;preset.append(option);}preset.value=settings.preset;
    const volume=node('input');volume.name='volume';volume.type='range';volume.min='0';volume.max='1';volume.step='.05';volume.value=String(settings.audio_volume);
    const latency=node('input');latency.name='latency';latency.type='number';latency.min='0';latency.max='8';latency.step='1';latency.value=String(settings.latency_frames);latency.required=true;
    const save=node('button','button button-secondary',text('save'));save.type='submit';
    form.append(field(text('preset'),preset),field(text('volume'),volume),field(text('latency'),latency),node('p','muted small',text('next')),save);
    form.addEventListener('submit',async event=>{event.preventDefault();save.disabled=true;try{await bridge.request('game_settings_set',{game_id:game.id,config:{...settings,preset:preset.value,audio_volume:Number(volume.value),latency_frames:Number(latency.value)}});notify(text('saved'));}catch(error){notify(error.message,true);}finally{save.disabled=false;}});
    const assistant=node('section','local-assistant');assistant.append(node('h3','',text('assistant')));
    if(!global.ai_enabled)assistant.append(node('p','muted small',text('disabled')));
    else{
      assistant.append(node('p','muted small',text('local')));
      const topic=node('select');topic.setAttribute('aria-label',text('assistant'));
      for(const key of['stutter','audio','controls','saves','battery','privacy']){const option=node('option','',text(key==='battery'?'batteryTopic':key));option.value=key;topic.append(option);}
      const ask=node('button','button button-secondary',text('ask'));ask.type='button';const answer=node('div','assistant-answer');answer.setAttribute('aria-live','polite');
      ask.addEventListener('click',async()=>{
        ask.disabled=true;
        try{
          const advice=await bridge.request('assistant_advise',{game_id:game.id,topic:topic.value});answer.replaceChildren(node('p','small',advice.message));
          if(advice.proposal){
            const actions=node('div','detail-actions'),accept=node('button','button button-primary',text('accept')),dismiss=node('button','button button-quiet',text('dismiss'));accept.type='button';dismiss.type='button';
            accept.addEventListener('click',async()=>{accept.disabled=true;try{const config=await bridge.request('assistant_accept',{proposal_id:advice.proposal.id});Object.assign(settings,config);preset.value=config.preset;volume.value=String(config.audio_volume);latency.value=String(config.latency_frames);actions.remove();notify(text('applied'));}catch(error){notify(error.message,true);}finally{accept.disabled=false;}});
            dismiss.addEventListener('click',async()=>{try{await bridge.request('assistant_dismiss',{proposal_id:advice.proposal.id});answer.replaceChildren();}catch(error){notify(error.message,true);}});
            actions.append(accept,dismiss);answer.append(actions);
          }
        }catch(error){notify(error.message,true);}finally{ask.disabled=false;}
      });assistant.append(topic,ask,answer);
    }
    container.replaceChildren(form,assistant);
  }catch(error){container.replaceChildren(node('p','muted small',error.message));}
}
