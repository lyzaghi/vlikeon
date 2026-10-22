// A user-started local measurement, not a conformance test or a device-lab attestation.
export class BenchmarkCapture extends EventTarget {
  #runtime;#identity;#state='idle';#warmUntil=0;#started=0;#timer=0;#samples=[];#totals;#nominal=0;#captureStarted=0;
  #onMetrics;#onAudio;
  get state(){return this.#state;}
  constructor(runtime,identity){
    super();this.#runtime=runtime;this.#identity=identity;
    this.#onMetrics=event=>this.#metric(event.detail);this.#onAudio=event=>{if(this.#state==='measuring')this.#totals.audio_underruns+=Math.max(0,Number(event.detail.underruns)||0);};
  }
  async start(){
    if(this.#state!=='idle'||this.#runtime.state!=='running')throw new Error('Start measurement during an active game.');
    await this.#runtime.collectMeasurement(true);
    this.#totals={frames_presented:0,frames_emulated:0,dropped_frames:0,audio_underruns:0};this.#samples=[];this.#nominal=0;this.#state='warming';this.#captureStarted=performance.now();this.#warmUntil=this.#captureStarted+10000;
    this.#runtime.addEventListener('metrics',this.#onMetrics);this.#runtime.addEventListener('audio_metrics',this.#onAudio);
    this.#timer=setInterval(()=>{if(this.#runtime.state!=='running')void this.cancel('Playback paused or stopped.');else if(performance.now()-this.#captureStarted>210000||(this.#started&&performance.now()-this.#started>180000))void this.cancel('The measurement exceeded its time bound.');},500);
    this.#progress();
  }
  #metric(metric){
    if(!['warming','measuring'].includes(this.#state))return;
    if(!(metric.frameIntervalsMs instanceof ArrayBuffer)||metric.frameIntervalsMs.byteLength>4000||!Number.isFinite(metric.windowDurationMs)||metric.windowDurationMs<=0||metric.windowDurationMs>10000){void this.cancel('The runtime returned an invalid measurement window.');return;}
    const now=performance.now(),windowStart=now-metric.windowDurationMs;
    if(this.#state==='warming'){
      if(windowStart<this.#warmUntil){this.#progress();return;}
      this.#state='measuring';this.#started=windowStart;this.#nominal=metric.nominalRefreshHz;
    }
    if(this.#nominal!==metric.nominalRefreshHz){void this.cancel('The nominal game clock changed during measurement.');return;}
    const values=new Float32Array(metric.frameIntervalsMs);
    if(this.#samples.length+values.length>36000||Array.from(values).some(v=>!Number.isFinite(v)||v<=0||v>60000)){void this.cancel('Measurement samples exceed their bound.');return;}
    this.#samples.push(...values);
    this.#totals.frames_presented+=metric.framesPresented;this.#totals.frames_emulated+=metric.framesEmulated;this.#totals.dropped_frames+=metric.droppedFrames;
    const elapsed=Math.round(now-this.#started);this.#progress(elapsed);
    if(elapsed>=120000){
      const report={id:crypto.randomUUID(),...this.#identity,nominal_refresh_hz:this.#nominal,observed_duration_ms:elapsed,warmup_ms:10000,...this.#totals,frame_intervals_ms:this.#samples};
      this.#state='complete';void this.#detach();this.dispatchEvent(new CustomEvent('complete',{detail:report}));
    }
  }
  #progress(elapsed=0){this.dispatchEvent(new CustomEvent('progress',{detail:{state:this.#state,elapsedMs:elapsed,targetMs:120000,warmupRemainingMs:Math.max(0,this.#warmUntil-performance.now())}}));}
  async cancel(reason='Measurement cancelled.'){if(!['warming','measuring'].includes(this.#state))return;this.#state='cancelled';await this.#detach();this.#samples=[];this.dispatchEvent(new CustomEvent('cancelled',{detail:{reason}}));}
  async #detach(){clearInterval(this.#timer);this.#runtime.removeEventListener('metrics',this.#onMetrics);this.#runtime.removeEventListener('audio_metrics',this.#onAudio);await this.#runtime.collectMeasurement(false).catch(()=>{});}
}
