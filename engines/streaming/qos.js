import{requireValue}from'./crypto.js';
export const PROFILES={p720_30:{width:1280,height:720,fps:30,bitrate:5000000},p720_60:{width:1280,height:720,fps:60,bitrate:8000000},p1080_30:{width:1920,height:1080,fps:30,bitrate:9000000},p1080_60:{width:1920,height:1080,fps:60,bitrate:14000000}};
export function validateMetrics(value){
  const fields=['rtt_ms','jitter_ms','packet_loss_percent','decode_ms','jitter_buffer_ms','frames_decoded_per_second','frames_dropped'];
  requireValue(value&&Object.keys(value).every(key=>fields.includes(key))&&fields.every(key=>value[key]===null||typeof value[key]==='number'&&Number.isFinite(value[key])&&value[key]>=0&&value[key]<=({packet_loss_percent:100,frames_decoded_per_second:300,frames_dropped:100000}[key]??60000)),'invalid_request','Streaming quality sample is invalid.');return value;
}
export class StreamQuality extends EventTarget{
  #sender;#ceiling;#current;#bad=0;#good=0;#last=0;#pending=null;#busy=false;#closed=false;
  constructor(sender,ceiling){super();this.#sender=sender;this.#ceiling=ceiling;this.#current=ceiling;}
  async sample(value){
    if(this.#closed)return;validateMetrics(value);if(this.#busy)return;this.#busy=true;
    try{
      const now=performance.now(),bad=(value.packet_loss_percent??0)>5||(value.rtt_ms??0)>180||(value.jitter_buffer_ms??0)>100||(value.frames_dropped??0)>10;
      this.#bad=bad?this.#bad+1:0;this.#good=bad?0:this.#good+1;
      if(this.#pending&&now-this.#last>=4000){
        const previous=this.#pending;this.#pending=null;
        if(previous.baseline.rtt_ms!==null&&value.rtt_ms!==null&&value.rtt_ms>previous.baseline.rtt_ms*1.3+20&&(value.frames_dropped??0)>(previous.baseline.frames_dropped??0)+5){await this.#set(previous.bitrate,'regression_rollback');this.#last=now+12000;this.#bad=this.#good=0;return;}
      }
      if(now-this.#last<8000||this.#pending)return;
      const next=this.#bad>=3?Math.max(2000000,Math.floor(this.#current*.8)):this.#good>=10?Math.min(this.#ceiling,Math.floor(this.#current*1.1)):this.#current;
      if(next===this.#current)return;const before=this.#current;await this.#set(next,next<before?'congestion_reduction':'healthy_recovery');if(this.#closed)return;this.#pending={bitrate:before,baseline:value};this.#last=now;this.#bad=this.#good=0;
    }finally{this.#busy=false;}
  }
  close(){this.#closed=true;this.#sender=null;this.#pending=null;}
  async #set(bitrate,reason){if(this.#closed)return;const sender=this.#sender,parameters=sender.getParameters();requireValue(Array.isArray(parameters.encodings)&&parameters.encodings.length===1,'unsupported_capability','The negotiated video sender cannot apply this quality policy.');parameters.encodings[0].maxBitrate=bitrate;await sender.setParameters(parameters);if(this.#closed||sender!==this.#sender)return;this.#current=bitrate;this.dispatchEvent(new CustomEvent('change',{detail:{bitrate,reason}}));}
}
export class StreamStats{
  #previous=new Map();
  async sample(pc,supplied){
    const report=supplied??await pc.getStats(),value={rtt_ms:null,jitter_ms:null,packet_loss_percent:null,decode_ms:null,jitter_buffer_ms:null,frames_decoded_per_second:null,frames_dropped:null};let next=new Map();
    for(const stat of report.values()){
      if(stat.type==='candidate-pair'&&stat.state==='succeeded'&&(stat.nominated||stat.selected)&&Number.isFinite(stat.currentRoundTripTime))value.rtt_ms=stat.currentRoundTripTime*1000;
      if(stat.type!=='inbound-rtp'||stat.isRemote||(stat.kind??stat.mediaType)!=='video')continue;
      next.set(stat.id,stat);const before=this.#previous.get(stat.id);if(Number.isFinite(stat.jitter))value.jitter_ms=stat.jitter*1000;if(!before)continue;const dt=(stat.timestamp-before.timestamp)/1000;if(dt<=0||dt>10)continue;
      const delta=key=>Number.isFinite(stat[key])&&Number.isFinite(before[key])?Math.max(0,stat[key]-before[key]):null,received=delta('packetsReceived'),lost=delta('packetsLost'),decoded=delta('framesDecoded'),decode=delta('totalDecodeTime'),emitted=delta('jitterBufferEmittedCount'),delay=delta('jitterBufferDelay');
      if(received!==null&&lost!==null&&received+lost>0)value.packet_loss_percent=100*lost/(received+lost);if(decoded!==null)value.frames_decoded_per_second=decoded/dt;if(decoded>0&&decode!==null)value.decode_ms=decode*1000/decoded;if(emitted>0&&delay!==null)value.jitter_buffer_ms=delay*1000/emitted;value.frames_dropped=delta('framesDropped');
    }this.#previous=next;return validateMetrics(value);
  }
}
