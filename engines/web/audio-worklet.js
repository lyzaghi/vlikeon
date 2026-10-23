class OmniPlayAudioProcessor extends AudioWorkletProcessor {
  constructor(){
    super();
    this.capacity=Math.ceil(sampleRate*.3)*2;
    this.buffer=new Float32Array(this.capacity);
    this.metricsSequence=0;this.metricsPending=0;
    this.readIndex=0;this.writeIndex=0;this.length=0;this.sourceRate=sampleRate;this.phase=0;this.volume=.8;this.underruns=0;this.droppedFrames=0;this.rendered=0;this.latencyFrames=2;this.nominalHz=60;this.primed=false;
    this.port.onmessage=event=>{
      const data=event.data||{};
      if(data.method==='metrics_ack'){if(data.sequence===this.metricsPending)this.metricsPending=0;return;}
      if(data.method==='clear'){this.length=0;this.phase=0;this.readIndex=this.writeIndex;this.primed=false;return;}
      if(data.method==='volume'){if(Number.isFinite(data.volume))this.volume=Math.min(1,Math.max(0,data.volume));return;}
      if(data.method==='configure'){if(Number.isInteger(data.latencyFrames)&&data.latencyFrames>=0&&data.latencyFrames<=8)this.latencyFrames=data.latencyFrames;if(Number.isFinite(data.nominalHz)&&data.nominalHz>=20&&data.nominalHz<=240)this.nominalHz=data.nominalHz;return;}
      if(data.method!=='samples')return;
      // Return the credit even for a rejected packet. At most four bounded
      // packets may be in flight; rendering does not keep the producer alive.
      const acknowledge=()=>{if(Number.isSafeInteger(data.sequence)&&data.sequence>0)this.port.postMessage({event:'audio_ack',sequence:data.sequence});};
      if(!(data.samples instanceof ArrayBuffer)||data.samples.byteLength===0||data.samples.byteLength>16384*2*4||data.samples.byteLength%8||!Number.isFinite(data.sampleRate)||data.sampleRate<8000||data.sampleRate>192000){acknowledge();return;}
      if(this.sourceRate!==data.sampleRate){this.readIndex=this.writeIndex;this.length=0;this.phase=0;this.primed=false;}
      this.sourceRate=data.sampleRate;
      const samples=new Float32Array(data.samples);
      if(samples.length%2)return;
      // Keep bounded latency: stale audio is less useful than the newest samples.
      const maxSamples=Math.min(this.capacity,Math.max(Math.ceil(this.sourceRate*.05)*2,Math.ceil(this.sourceRate/this.nominalHz*(this.latencyFrames+2))*2));
      for(let i=0;i<samples.length;i+=2){
        while(this.length+2>maxSamples&&this.length>=2){this.readIndex=(this.readIndex+2)%this.capacity;this.length-=2;this.phase=0;this.droppedFrames++;}
        this.buffer[this.writeIndex]=Math.max(-1,Math.min(1,Number.isFinite(samples[i])?samples[i]:0));this.buffer[(this.writeIndex+1)%this.capacity]=Math.max(-1,Math.min(1,Number.isFinite(samples[i+1])?samples[i+1]:0));this.writeIndex=(this.writeIndex+2)%this.capacity;this.length+=2;
      }
      acknowledge();
    };
  }
  process(inputs,outputs){
    const output=outputs[0];if(!output?.length)return true;
    const left=output[0],right=output[1]||output[0],ratio=this.sourceRate/sampleRate;
    const targetSamples=Math.min(this.capacity-4,Math.max(128,Math.ceil(this.sourceRate/this.nominalHz*this.latencyFrames))*2);
    if(!this.primed&&this.length>=targetSamples)this.primed=true;
    if(!this.primed){left.fill(0);right.fill(0);return true;}
    let starved=false;
    for(let index=0;index<left.length;index++){
      if(this.length<4){left[index]=0;right[index]=0;starved=true;continue;}
      const next=(this.readIndex+2)%this.capacity;
      left[index]=(this.buffer[this.readIndex]*(1-this.phase)+this.buffer[next]*this.phase)*this.volume;
      right[index]=(this.buffer[(this.readIndex+1)%this.capacity]*(1-this.phase)+this.buffer[(next+1)%this.capacity]*this.phase)*this.volume;
      this.phase+=ratio;
      while(this.phase>=1&&this.length>=2){this.phase-=1;this.readIndex=(this.readIndex+2)%this.capacity;this.length-=2;}
    }
    if(starved){this.underruns++;this.primed=false;this.phase=0;this.length=0;this.readIndex=this.writeIndex;}
    this.rendered+=left.length;
    if(this.rendered>=sampleRate&&!this.metricsPending&&this.metricsSequence<Number.MAX_SAFE_INTEGER){this.metricsPending=++this.metricsSequence;this.port.postMessage({event:'audio_metrics',sequence:this.metricsPending,underruns:this.underruns,droppedFrames:this.droppedFrames,queueMs:this.length/2/this.sourceRate*1000});this.underruns=0;this.droppedFrames=0;this.rendered=0;}
    return true;
  }
}
registerProcessor('omniplay-audio',OmniPlayAudioProcessor);
