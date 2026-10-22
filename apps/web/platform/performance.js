import{UUID,HASH,integer,utf8,requireValue,fail}from'./errors.js';
import{Sha256}from'./sha256.js';

export class WebPerformance {
  #store;#catalog;
  constructor(store,catalog){this.#store=store;this.#catalog=catalog;}
  async commit(input,{signal}={}){
    const fields=['id','game_id','game_content_hash','runtime_id','runtime_version','source_revision','runtime_build_hash','core_config_hash','preset','nominal_refresh_hz','observed_duration_ms','warmup_ms','frames_presented','frames_emulated','dropped_frames','audio_underruns','frame_intervals_ms'];
    requireValue(input&&Object.keys(input).every(key=>fields.includes(key))&&UUID.test(input.id)&&UUID.test(input.game_id)&&[input.game_content_hash,input.runtime_build_hash,input.core_config_hash].every(value=>typeof value==='string'&&HASH.test(value)),'invalid_request','Measurement identity is invalid.');
    const core=this.#catalog.find(value=>value.id===input.runtime_id);
    requireValue(core?.distribution_approved===true&&core.security_revoked!==true&&input.runtime_version===core.core_version&&input.source_revision===core.source_revision&&input.runtime_build_hash===core.wasm_sha256,'untrusted_artifact','Measurement runtime identity does not match an approved bundled core.');
    requireValue(['accurate','smooth','battery'].includes(input.preset)&&Number.isFinite(input.nominal_refresh_hz)&&input.nominal_refresh_hz>=20&&input.nominal_refresh_hz<=240&&integer(input.observed_duration_ms,120000,180000)&&integer(input.warmup_ms,10000,60000)&&['frames_presented','frames_emulated','dropped_frames','audio_underruns'].every(key=>integer(input[key],0,0xffffffff))&&input.frames_emulated>=100&&input.frames_presented<=input.frames_emulated*4&&Array.isArray(input.frame_intervals_ms)&&input.frame_intervals_ms.length>=100&&input.frame_intervals_ms.length<=36000&&input.frame_intervals_ms.every(value=>Number.isFinite(value)&&value>0&&value<=60000),'invalid_request','Measurement samples are incomplete or outside their bounds.');
    const raw=JSON.stringify(input),bytes=utf8(raw);requireValue(bytes.length<=768*1024,'resource_limit','Measurement samples exceed their storage bound.');const hash=new Sha256().update(bytes).hex(),total=input.frame_intervals_ms.reduce((a,b)=>a+b,0);
    requireValue(Math.abs(total-input.observed_duration_ms)<=3000,'invalid_request','Measurement samples do not cover their observed duration.');const sorted=[...input.frame_intervals_ms].sort((a,b)=>a-b),quantile=q=>sorted[Math.floor((sorted.length-1)*q)],seconds=input.observed_duration_ms/1000;
    const{frame_intervals_ms,...identity}=input,summary={...identity,created_at_ms:Date.now(),platform:'web-pwa',device_class:'unclassified',frame_time_p50_ms:quantile(.5),frame_time_p95_ms:quantile(.95),frame_time_p99_ms:quantile(.99),frames_per_second:input.frames_emulated/seconds,presented_frames_per_second:input.frames_presented/seconds,measurement_source:'web_worker_performance_clock',independently_attested:false};
    return this.#store.transaction(['games','performance'],'readwrite',async tx=>{
      const game=await tx.get('games',input.game_id);requireValue(game?.content_hash===input.game_content_hash,'content_mismatch','Measurement belongs to different library content.');const previous=await tx.get('performance',input.id);if(previous){requireValue(previous.input_hash===hash,'conflict','Measurement identity was reused with different samples.');return previous.summary;}
      await tx.add('performance',{id:input.id,game_id:input.game_id,created_at_ms:summary.created_at_ms,input_hash:hash,input:structuredClone(input),summary});
      const all=await tx.all('performance');all.sort((a,b)=>b.created_at_ms-a.created_at_ms||(a.id<b.id?1:-1));for(const old of all.slice(20))await tx.delete('performance',old.id);return summary;
    },signal);
  }
  async list(gameId){requireValue(UUID.test(gameId),'invalid_request','Game identity is invalid.');return this.#store.transaction(['performance'],'readonly',async tx=>{const all=await tx.request(tx.store('performance').index('game').getAll(gameId));return all.sort((a,b)=>b.created_at_ms-a.created_at_ms).slice(0,20).map(value=>value.summary);});}
  async #record(id){requireValue(UUID.test(id),'invalid_request','Measurement identity is invalid.');const record=await this.#store.transaction(['performance'],'readonly',tx=>tx.get('performance',id));if(!record)fail('not_found','Measurement was not found.');return record;}
  async compare(baselineId,candidateId){
    const baseline=(await this.#record(baselineId)).summary,candidate=(await this.#record(candidateId)).summary;
    requireValue(['game_content_hash','runtime_id','platform','nominal_refresh_hz','measurement_source'].every(key=>baseline[key]===candidate[key]),'invalid_request','Compare the same content, runtime, clock, platform and measurement source.');
    return{baseline,candidate,frame_time_p95_change_percent:(candidate.frame_time_p95_ms/baseline.frame_time_p95_ms-1)*100,audio_underrun_change:candidate.audio_underruns-baseline.audio_underruns,controlled_hardware_comparison:false};
  }
  async export(id){const record=await this.#record(id);return new Blob([JSON.stringify({schema_version:1,summary:record.summary,measurements:record.input},null,2)],{type:'application/json'});}
  async remove(id,{signal}={}){requireValue(UUID.test(id),'invalid_request','Measurement identity is invalid.');await this.#store.transaction(['performance'],'readwrite',tx=>tx.delete('performance',id),signal);return{deleted:true};}
  async clear({signal}={}){await this.#store.transaction(['performance'],'readwrite',tx=>tx.request(tx.store('performance').clear()),signal);return{deleted:true};}
}
