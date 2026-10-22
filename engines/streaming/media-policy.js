import{requireValue}from'./crypto.js';

export const CODEC_POLICIES=Object.freeze({auto:'Auto',h264:'H.264',vp8:'VP8',vp9:'VP9',av1:'AV1',hevc:'H.265 / HEVC'});
const MIME=Object.freeze({h264:'video/h264',vp8:'video/vp8',vp9:'video/vp9',av1:'video/av1',hevc:'video/h265'});
function capabilities(role){
  const api=role==='host'?globalThis.RTCRtpSender:globalThis.RTCRtpReceiver;
  let value;try{value=api?.getCapabilities?.('video')?.codecs;}catch{return[];}
  if(!Array.isArray(value)||value.length>128)return[];
  return value.filter(item=>item&&typeof item.mimeType==='string'&&/^video\/[a-z0-9-]{1,32}$/i.test(item.mimeType)&&
    Number.isFinite(item.clockRate)&&item.clockRate>0&&item.clockRate<=192000&&
    (item.sdpFmtpLine===undefined||typeof item.sdpFmtpLine==='string'&&item.sdpFmtpLine.length<=2048));
}
export function availableCodecPolicies(){
  if(typeof globalThis.RTCRtpTransceiver?.prototype?.setCodecPreferences!=='function')return['auto'];
  const types=new Set([...capabilities('host'),...capabilities('client')].map(item=>item.mimeType.toLowerCase()));
  return['auto',...Object.keys(MIME).filter(key=>types.has(MIME[key]))];
}
export function preferVideoCodec(pc,policy,role){
  requireValue(Object.hasOwn(CODEC_POLICIES,policy),'invalid_request','The video codec preference is invalid.');
  if(policy==='auto')return;
  const all=capabilities(role),preferred=all.filter(item=>item.mimeType.toLowerCase()===MIME[policy]);
  requireValue(preferred.length>0,'unsupported_capability','This device cannot use that codec preference in this role. Choose Auto or another codec.');
  // Reorder actual capability objects. Keep the browser's remaining primary and
  // RTX/RED/FEC entries; this is a preference, never an invented SDP codec or an
  // assertion that the codec can be encoded with hardware acceleration.
  const order=[...preferred,...all.filter(item=>!preferred.includes(item))];
  let applied=false;
  for(const transceiver of pc.getTransceivers()){
    if((transceiver.sender.track?.kind??transceiver.receiver.track?.kind)!=='video')continue;
    requireValue(typeof transceiver.setCodecPreferences==='function','unsupported_capability','Codec preferences are unavailable in this browser.');
    transceiver.setCodecPreferences(order);applied=true;
  }
  requireValue(applied,'invalid_state','The remote session has no video transceiver.');
}

const number=(value,maximum)=>typeof value==='number'&&Number.isFinite(value)&&value>=0&&value<=maximum?value:null;
const delta=(now,before,key)=>number(now[key],Number.MAX_SAFE_INTEGER)!==null&&number(before?.[key],Number.MAX_SAFE_INTEGER)!==null&&now[key]>=before[key]?now[key]-before[key]:null;
export class MediaObservation{
  #previous=null;
  sample(report,role){
    const type=role==='host'?'outbound-rtp':'inbound-rtp';let selected=null,count=0;
    for(const stat of report.values()){
      if(++count>2048)break;
      if(stat.type===type&&!stat.isRemote&&(stat.kind??stat.mediaType)==='video'&&
        (!selected||(stat[role==='host'?'bytesSent':'bytesReceived']??0)>(selected[role==='host'?'bytesSent':'bytesReceived']??0)))selected=stat;
    }
    if(!selected){this.#previous=null;return{protocol:1,direction:role==='host'?'send':'receive',codec:null,width:null,height:null,fps:null,
      bitrate_bps:null,processing_ms:null,power_efficient:null,limitation:null};}
    const codec=report.get(selected.codecId),mime=typeof codec?.mimeType==='string'&&/^video\/[a-z0-9-]{1,32}$/i.test(codec.mimeType)?codec.mimeType:null;
    let bitrate=null,processing=null;
    const before=this.#previous,dt=before?.id===selected.id&&before.type===selected.type?(selected.timestamp-before.timestamp)/1000:0;
    if(dt>0&&dt<=10){
      const bytes=delta(selected,before,role==='host'?'bytesSent':'bytesReceived'),frames=delta(selected,before,role==='host'?'framesEncoded':'framesDecoded'),
        duration=delta(selected,before,role==='host'?'totalEncodeTime':'totalDecodeTime');
      if(bytes!==null)bitrate=number(bytes*8/dt,1000000000);
      if(frames>0&&duration!==null)processing=number(duration*1000/frames,60000);
    }
    // Retain only the counters used for the next sample, not device labels,
    // candidate addresses, source tokens or implementation fingerprint strings.
    this.#previous={id:selected.id,type:selected.type,timestamp:selected.timestamp,bytesSent:selected.bytesSent,bytesReceived:selected.bytesReceived,
      framesEncoded:selected.framesEncoded,framesDecoded:selected.framesDecoded,totalEncodeTime:selected.totalEncodeTime,totalDecodeTime:selected.totalDecodeTime};
    const efficient=selected[role==='host'?'powerEfficientEncoder':'powerEfficientDecoder'];
    return{protocol:1,direction:role==='host'?'send':'receive',codec:mime,width:number(selected.frameWidth,8192),height:number(selected.frameHeight,8192),
      fps:number(selected.framesPerSecond,240),bitrate_bps:bitrate,processing_ms:processing,
      power_efficient:typeof efficient==='boolean'?efficient:null,
      limitation:role==='host'&&['none','cpu','bandwidth','other'].includes(selected.qualityLimitationReason)?selected.qualityLimitationReason:null};
  }
}
