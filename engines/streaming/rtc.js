import{StreamError,requireValue,UUID,bytes,exactKeys}from'./crypto.js';
import{PROFILES,StreamQuality,StreamStats,validateMetrics}from'./qos.js';
import{validateInput}from'./input.js';
import{CaptureRequest,CaptureSource}from'./capture.js';
import{CODEC_POLICIES,preferVideoCodec,MediaObservation}from'./media-policy.js';

function description(value,type){requireValue(exactKeys(value,['type','sdp'])&&value.type===type&&typeof value.sdp==='string'&&bytes(value.sdp).length<=44*1024&&value.sdp.includes('m=video ')&&value.sdp.includes('a=fingerprint:sha-256 '),'invalid_request','Remote media description is invalid.');const fingerprints=value.sdp.split(/\r?\n/).filter(line=>line.startsWith('a=fingerprint:'));requireValue(fingerprints.length>0&&fingerprints.length<=8&&fingerprints.every(line=>/^a=fingerprint:sha-256 (?:[A-Fa-f0-9]{2}:){31}[A-Fa-f0-9]{2}$/.test(line)),'unauthorized','Only authenticated SHA-256 media fingerprints are admitted.');return{type,sdp:value.sdp};}
function candidate(value){if(value===null)return null;requireValue(value&&typeof value.candidate==='string'&&value.candidate.length<=4096&&(value.sdpMid===null||typeof value.sdpMid==='string'&&value.sdpMid.length<=64)&&(value.sdpMLineIndex===null||Number.isSafeInteger(value.sdpMLineIndex)&&value.sdpMLineIndex>=0&&value.sdpMLineIndex<=7)&&(value.usernameFragment===null||value.usernameFragment===undefined||typeof value.usernameFragment==='string'&&value.usernameFragment.length<=256),'invalid_request','Remote network candidate is invalid.');return{candidate:value.candidate,sdpMid:value.sdpMid,sdpMLineIndex:value.sdpMLineIndex,usernameFragment:value.usernameFragment??undefined};}
function iceServers(value){return value.map(server=>{requireValue(server&&Array.isArray(server.urls)&&server.urls.length>0&&server.urls.length<=8&&server.urls.every(url=>typeof url==='string'&&url.length<=256&&/^turns?:[A-Za-z0-9.:[\]-]+(?:\?transport=(?:udp|tcp))?$/.test(url))&&typeof server.username==='string'&&server.username.length<=256&&typeof server.credential==='string'&&server.credential.length<=256,'invalid_request','TURN server authorization is invalid.');return{urls:server.urls,username:server.username,credential:server.credential,credentialType:'password'};});}
export class RemoteSession extends EventTarget{
  #transport;#pc=null;#session=null;#role;#profile;#relayOnly;#stream=null;#remote=new MediaStream();#control=null;#input=null;#quality=null;#stats=new StreamStats();#timer=0;#statsBusy=false;#queue=Promise.resolve();#candidates=[];#localCandidates=[];#localReady=false;#candidateCount=0;#generation=0;#remoteGeneration=0;#pending=null;#closed=false;#restarts=0;#reconnectTimer=0;#negotiationTimer=0;#lastQuality=0;#inputSequence=0;#inputArmed=false;#remoteInputEnabled=false;#inputTimer=0;#lastInputAt=0;
  #queuedSignals=0;#connecting=false;#accepting=false;#negotiating=false;#recovering=false;#stopPromise=null;
  #remoteGamepadEnabled=false;
  #captureRequest=null;#captureSource=null;#lifetime=new AbortController();#codec='auto';#media=new MediaObservation();
  #inputEpoch=1;#remoteInputEpoch=0;#inputHasFrames=false;
  constructor(transport,{profile='p720_60',relayOnly=false,codec='auto'}={}){
    super();requireValue(PROFILES[profile]&&Object.hasOwn(CODEC_POLICIES,codec),'invalid_request','Stream profile or codec preference is invalid.');this.#transport=transport;this.#role=transport.pair.role;this.#profile=profile;this.#relayOnly=relayOnly;this.#codec=codec;
    globalThis.addEventListener('pagehide',()=>void this.stop('host_page_closed'),{once:true,signal:this.#lifetime.signal});
    globalThis.document?.addEventListener('freeze',()=>void this.stop('host_page_suspended'),{once:true,signal:this.#lifetime.signal});
    transport.addEventListener('signal',event=>{if(this.#closed)return;if(++this.#queuedSignals>64){this.#error(new StreamError('resource_limit','Pending session negotiation exceeds its bound.'));return;}this.#queue=this.#queue.then(()=>this.#signal(event.detail)).catch(error=>this.#error(error)).finally(()=>{this.#queuedSignals--;});},{signal:this.#lifetime.signal});transport.addEventListener('transport_error',event=>this.#error(new StreamError(event.detail.code,event.detail.message)),{signal:this.#lifetime.signal});
  }
  get active(){return Boolean(this.#pc)&&!this.#closed;}
  get role(){return this.#role;}
  get id(){return this.#session??this.#pending?.session_id??null;}
  get remoteStream(){return this.#remote;}
  get localStream(){return this.#stream;}
  get gamepadInputAvailable(){return this.#role==='client'&&this.#remoteInputEnabled&&this.#remoteGamepadEnabled;}
  get captureSnapshot(){return this.#captureSource?.snapshot()??null;}
  async #connection(){
    requireValue(typeof RTCPeerConnection==='function','unsupported_capability','This browser cannot create the required real-time connection.');requireValue(!this.#closed&&!this.#pc&&!this.#connecting,'invalid_state','This session cannot create another real-time connection.');this.#connecting=true;
    try{
    const ice=await this.#transport.ice();requireValue(!this.#relayOnly||ice.relay_available,'unsupported_capability','Relay-only play requires an authorized TURN server.');
    requireValue(!this.#closed,'cancelled','The remote session was closed.');const pc=new RTCPeerConnection({iceServers:iceServers(ice.ice_servers),iceTransportPolicy:this.#relayOnly?'relay':'all',bundlePolicy:'max-bundle',rtcpMuxPolicy:'require'});this.#pc=pc;
    pc.onicecandidate=event=>{if(this.#closed||!this.#session)return;if(++this.#candidateCount>256){this.#error(new StreamError('resource_limit','Network candidate limit reached.'));return;}const value={type:'candidate',generation:this.#generation,candidate:event.candidate?event.candidate.toJSON():null};if(!this.#localReady)this.#localCandidates.push(value);else void this.#transport.send(this.#session,value).catch(error=>this.#error(error));};
    pc.onconnectionstatechange=()=>{
      if(this.#closed||pc!==this.#pc)return;
      this.dispatchEvent(new CustomEvent('state',{detail:{state:pc.connectionState}}));
      if(pc.connectionState==='connected'){clearTimeout(this.#reconnectTimer);this.#reconnectTimer=0;clearTimeout(this.#negotiationTimer);this.#restarts=0;}
      else if(['disconnected','failed'].includes(pc.connectionState)){this.armInput(false);if(!this.#reconnectTimer)this.#reconnectTimer=setTimeout(()=>{this.#reconnectTimer=0;void this.#recover();},pc.connectionState==='failed'?500:3000);}
    };
    pc.ontrack=event=>{if(this.#closed||pc!==this.#pc||this.#role!=='client'||!['video','audio'].includes(event.track.kind)){event.track.stop();return;}const same=this.#remote.getTracks().filter(track=>track.kind===event.track.kind);for(const track of same){this.#remote.removeTrack(track);track.stop();}this.#remote.addTrack(event.track);this.dispatchEvent(new CustomEvent('media',{detail:{stream:this.#remote}}));};
    pc.ondatachannel=event=>{if(this.#closed||pc!==this.#pc||this.#role!=='host'){event.channel.close();return;}this.#channel(event.channel);};
    this.#timer=setInterval(()=>void this.#sample(),1000);return pc;
    }finally{this.#connecting=false;}
  }
  #live(pc){requireValue(Boolean(pc)&&!this.#closed&&pc===this.#pc,'cancelled','The remote connection was closed or replaced.');return pc;}
  async connect(){
    requireValue(this.#role==='client'&&!this.#pc&&!this.#connecting&&!this.#closed,'invalid_state','This client session cannot connect.');this.#session=crypto.randomUUID();this.#deadline();
    try{const pc=this.#live(await this.#connection());pc.addTransceiver('video',{direction:'recvonly'});pc.addTransceiver('audio',{direction:'recvonly'});this.#channel(pc.createDataChannel('omniplay-control-v2',{ordered:true,protocol:'omniplay.control.v2'}));this.#channel(pc.createDataChannel('omniplay-input-v2',{ordered:false,maxRetransmits:0,protocol:'omniplay.input.v2'}));await this.#offer(false);}
    catch(error){await this.stop('connection_failed');throw error;}
  }
  async #offer(restart){
    const pc=this.#pc;requireValue(pc&&!this.#closed&&!this.#negotiating&&this.#role==='client'&&pc.signalingState==='stable','invalid_state','The client cannot negotiate in this state.');requireValue(this.#generation<16,'resource_limit','Start a new remote session before negotiating again.');this.#negotiating=true;
    try{this.#deadline();this.#generation++;this.#candidateCount=0;this.#localReady=false;this.#localCandidates=[];preferVideoCodec(pc,this.#codec,this.#role);const offer=await pc.createOffer({iceRestart:restart});this.#live(pc);await pc.setLocalDescription(offer);this.#live(pc);await this.#transport.send(this.#session,{type:'offer',generation:this.#generation,description:{type:'offer',sdp:pc.localDescription.sdp},profile:this.#profile});this.#live(pc);await this.#flushLocal();}
    finally{this.#negotiating=false;}
  }
  #deadline(){clearTimeout(this.#negotiationTimer);this.#negotiationTimer=setTimeout(()=>this.#error(new StreamError('deadline_exceeded','The remote session did not finish negotiating in time.')),45000);}
  async #flushLocal(){while(this.#localCandidates.length){this.#live(this.#pc);const next=this.#localCandidates.shift();await this.#transport.send(this.#session,next);}this.#live(this.#pc);this.#localReady=true;}
  capture(options={}){
    const pending=this.#pending;
    requireValue(this.#role==='host'&&pending&&!this.#pc&&!this.#closed&&!this.#captureRequest,'invalid_state','Select a source for the current remote request.');
    const request=new CaptureRequest(pending.value.profile,{...options,signal:this.#lifetime.signal});this.#captureRequest=request;
    return request.result.then(source=>{
      if(this.#closed||this.#pending!==pending||this.#captureRequest!==request){source.stop();throw new StreamError('cancelled','The remote capture request was closed.');}
      return this.accept(source);
    }).finally(()=>{if(this.#captureRequest===request)this.#captureRequest=null;});
  }
  async accept(source){
    const pending=this.#pending;
    try{requireValue(this.#role==='host'&&pending&&!this.#pc&&!this.#accepting&&!this.#connecting&&!this.#closed&&source instanceof CaptureSource&&!source.closed&&source.snapshot().profile===pending.value.profile,'invalid_state','Select a capture source for the pending remote request.');}
    catch(error){if(source instanceof CaptureSource&&source!==this.#captureSource)source.stop();throw error;}
    this.#accepting=true;this.#pending=null;this.#session=pending.session_id;this.#profile=pending.value.profile;
    const stream=source.stream;this.#captureSource=source;this.#stream=stream;
    source.addEventListener('ended',()=>void this.stop('capture_ended'),{once:true,signal:this.#lifetime.signal});
    source.addEventListener('change',event=>this.dispatchEvent(new CustomEvent('capture',{detail:event.detail})),{signal:this.#lifetime.signal});
    this.dispatchEvent(new CustomEvent('capture',{detail:source.snapshot()}));
    try{
      requireValue(!this.#closed&&!source.closed,'cancelled','The capture source was closed.');this.#deadline();
      const pc=this.#live(await this.#connection()),config=PROFILES[this.#profile];for(const track of stream.getTracks()){requireValue(['video','audio'].includes(track.kind),'invalid_request','Unexpected capture track.');const sender=pc.addTrack(track,stream);if(track.kind==='video'){track.contentHint='motion';this.#quality=new StreamQuality(sender,config.bitrate);this.#quality.addEventListener('change',event=>this.dispatchEvent(new CustomEvent('quality_change',{detail:event.detail})),{signal:this.#lifetime.signal});}}
      await this.#answer(pending.value);
      this.#live(pc);
      const sender=pc.getSenders().find(sender=>sender.track?.kind==='video');if(sender){const parameters=sender.getParameters();if(parameters.encodings?.length===1){parameters.encodings[0].maxBitrate=config.bitrate;parameters.encodings[0].maxFramerate=config.fps;await sender.setParameters(parameters);}}
      this.#live(pc);requireValue(!source.closed,'cancelled','The capture source was closed.');
      return source.snapshot();
    }catch(error){source.stop();await this.stop('capture_failed');throw error;}finally{this.#accepting=false;}
  }
  async decline(){const pending=this.#pending;this.#pending=null;this.#captureRequest?.cancel();if(pending)await this.#transport.send(pending.session_id,{type:'stop',reason:'host_declined'});}
  async #answer(value){
    const pc=this.#live(this.#pc);requireValue(pc&&!this.#negotiating,'invalid_state','Media negotiation is already in progress.');requireValue(Number.isSafeInteger(value.generation)&&value.generation>this.#remoteGeneration&&value.generation<=16,'invalid_request','Offer generation is invalid.');this.#negotiating=true;
    try{this.#remoteGeneration=value.generation;this.#generation=value.generation;this.#candidateCount=0;
    this.#localReady=false;this.#localCandidates=[];await pc.setRemoteDescription(description(value.description,'offer'));this.#live(pc);await this.#drain();this.#live(pc);preferVideoCodec(pc,this.#codec,this.#role);const answer=await pc.createAnswer();this.#live(pc);await pc.setLocalDescription(answer);this.#live(pc);await this.#transport.send(this.#session,{type:'answer',generation:this.#generation,description:{type:'answer',sdp:pc.localDescription.sdp}});this.#live(pc);await this.#flushLocal();
    }finally{this.#negotiating=false;}
  }
  async #signal({session_id,value}){
    if(this.#closed)return;requireValue(UUID.test(session_id)&&typeof value.type==='string','invalid_request','Streaming session signal is invalid.');
    if(value.type==='offer'&&this.#role==='host'){
      description(value.description,'offer');requireValue(PROFILES[value.profile]&&Number.isSafeInteger(value.generation)&&value.generation>=1&&value.generation<=16,'invalid_request','Streaming offer profile is invalid.');
      if(!this.#pc&&!this.#pending&&!this.#connecting&&!this.#accepting){this.#pending={session_id,value};this.#deadline();this.dispatchEvent(new CustomEvent('incoming',{detail:{session_id,profile:value.profile}}));return;}
      if(this.#pc&&session_id===this.#session){await this.#answer(value);return;}
      await this.#transport.send(session_id,{type:'stop',reason:'host_busy'});return;
    }
    if(session_id!==this.#session&&session_id!==this.#pending?.session_id)return;
    if(value.type==='candidate'){
      requireValue(Number.isSafeInteger(value.generation)&&value.generation>=1&&value.generation<=16&&this.#candidates.length<256,'resource_limit','Remote candidate queue exceeds its bound.');const entry={generation:value.generation,candidate:candidate(value.candidate)};
      if(!this.#pc?.remoteDescription||value.generation!==this.#remoteGeneration)this.#candidates.push(entry);else await this.#pc.addIceCandidate(entry.candidate);return;
    }
    if(value.type==='answer'&&this.#role==='client'){
      const pc=this.#live(this.#pc);requireValue(pc&&value.generation===this.#generation&&pc.signalingState==='have-local-offer','invalid_state','Unexpected remote media answer.');await pc.setRemoteDescription(description(value.description,'answer'));this.#live(pc);this.#remoteGeneration=value.generation;await this.#drain();return;
    }
    if(value.type==='stop'){await this.stop('peer_closed',false);return;}
    if(value.type==='restart'&&this.#role==='client'){await this.#recover();return;}
    throw new StreamError('invalid_request','Unexpected streaming negotiation signal.');
  }
  async #drain(){const pc=this.#live(this.#pc),all=this.#candidates;this.#candidates=[];for(const entry of all){this.#live(pc);if(entry.generation===this.#remoteGeneration)await pc.addIceCandidate(entry.candidate);else if(entry.generation>this.#remoteGeneration)this.#candidates.push(entry);}this.#live(pc);}
  async #recover(){if(this.#closed||!this.#pc||this.#recovering||this.#negotiating||this.#role==='client'&&this.#pc.signalingState!=='stable')return;if(++this.#restarts>3){await this.stop('reconnect_limit');return;}const pc=this.#pc;this.#recovering=true;try{const ice=await this.#transport.ice();this.#live(pc);pc.setConfiguration({...pc.getConfiguration(),iceServers:iceServers(ice.ice_servers)});if(this.#role==='client')await this.#offer(true);else{this.#deadline();await this.#transport.send(this.#session,{type:'restart'});}}catch(error){this.#error(error);}finally{this.#recovering=false;}}
  #channel(channel){
    const control=channel.label==='omniplay-control-v2'&&channel.protocol==='omniplay.control.v2'&&channel.ordered, input=channel.label==='omniplay-input-v2'&&channel.protocol==='omniplay.input.v2'&&!channel.ordered&&channel.maxRetransmits===0;
    if(!control&&!input||control&&this.#control||input&&this.#input){channel.close();this.#error(new StreamError('unsupported_protocol','The remote control channel version is unsupported. Update both devices.'));return;}if(control)this.#control=channel;else this.#input=channel;
    channel.onmessage=event=>{
      if(this.#closed)return;
      try{requireValue(typeof event.data==='string'&&bytes(event.data).length<=8192,'resource_limit','Streaming data channel message exceeds its bound.');const value=JSON.parse(event.data);requireValue(value?.session_id===this.#session,'unauthorized','Data-channel session identity changed.');
        if(control&&value.type==='quality'&&this.#role==='host'){const now=performance.now();if(now-this.#lastQuality<500)return;this.#lastQuality=now;void this.#quality?.sample(validateMetrics(value.metrics)).catch(()=>{});}
        else if(control&&value.type==='release_input'&&this.#role==='host'){
          requireValue(Number.isSafeInteger(value.epoch)&&value.epoch>0&&value.epoch<=1000000&&Number.isSafeInteger(value.sequence)&&value.sequence>0&&value.sequence<=10000000,'invalid_request','The remote input release fence is invalid.');
          if(value.epoch!==this.#inputEpoch||value.sequence<=this.#inputSequence)return;
          this.#inputSequence=value.sequence;this.#releaseInput();
        }
        else if(control&&value.type==='input_policy'&&this.#role==='client'){
          requireValue(typeof value.enabled==='boolean'&&typeof value.gamepad==='boolean'&&Number.isSafeInteger(value.epoch)&&value.epoch>0&&value.epoch<=1000000,'invalid_request','Remote input policy is invalid.');
          requireValue(!value.enabled||this.#transport.pair.input_allowed,'unauthorized','The pairing does not authorize remote input.');
          if(value.epoch<this.#remoteInputEpoch)return;
          requireValue(value.epoch!==this.#remoteInputEpoch||value.enabled===this.#remoteInputEnabled&&value.gamepad===this.#remoteGamepadEnabled,'invalid_state','The remote input policy changed without a fresh epoch.');
          this.#remoteInputEpoch=value.epoch;this.#remoteInputEnabled=value.enabled;this.#remoteGamepadEnabled=value.gamepad;
          this.dispatchEvent(new CustomEvent('input_available',{detail:{available:this.#remoteInputEnabled&&this.#input?.readyState==='open',gamepad:this.#remoteGamepadEnabled}}));
        }
        else if(input&&value.type==='input'&&this.#role==='host'){
          requireValue(Number.isSafeInteger(value.epoch)&&value.epoch>0&&value.epoch<=1000000&&Number.isSafeInteger(value.sequence)&&value.sequence>0&&value.sequence<=10000000,'unauthorized','Remote input sequence or epoch is invalid.');
          if(value.epoch!==this.#inputEpoch||value.sequence<=this.#inputSequence)return;
          this.#inputSequence=value.sequence;const inputValue=validateInput(value.input),now=performance.now();
          if(!this.#inputArmed||!this.#transport.pair.input_allowed||now-this.#lastInputAt<5)return;
          this.#lastInputAt=now;this.#inputHasFrames=true;this.dispatchEvent(new CustomEvent('input',{detail:inputValue}));
        }
        else throw new StreamError('invalid_request','Unexpected streaming data message.');
      }catch(error){this.#error(error);}
    };
    channel.onclose=()=>{if(this.#closed)return;this.#remoteInputEnabled=false;this.#releaseInput();if(this.#closed)return;this.dispatchEvent(new CustomEvent('input_available',{detail:{available:false}}));if(!this.#closed){if(control)void this.stop('control_channel_closed');else if(this.#role==='host'){this.armInput(false);if(!this.#closed)this.dispatchEvent(new Event('input_expired'));}}};channel.onopen=()=>{if(this.#closed)return;if(control&&this.#role==='host')this.#policy();this.dispatchEvent(new CustomEvent('input_available',{detail:{available:this.#role==='client'&&this.#remoteInputEnabled&&this.#input?.readyState==='open'}}));};
  }
  sendInput(input){if(!this.#remoteInputEnabled||!this.#remoteInputEpoch||!this.#transport.pair.input_allowed||this.#role!=='client'||this.#input?.readyState!=='open'||this.#input.bufferedAmount>16384)return false;validateInput(input);requireValue(this.#inputSequence<10000000,'resource_limit','The input sequence requires a new session.');const value=JSON.stringify({type:'input',session_id:this.#session,epoch:this.#remoteInputEpoch,sequence:++this.#inputSequence,input:{...input,pads:this.#remoteGamepadEnabled?input.pads:[]}});requireValue(bytes(value).length<=8192,'resource_limit','Input state exceeds its bound.');this.#input.send(value);return true;}
  releaseInput(){if(this.#role==='client'&&this.#remoteInputEpoch&&this.#inputSequence<10000000&&this.#control?.readyState==='open'&&this.#control.bufferedAmount<16384)try{this.#control.send(JSON.stringify({type:'release_input',session_id:this.#session,epoch:this.#remoteInputEpoch,sequence:++this.#inputSequence}));}catch{}}
  armInput(enabled){
    clearInterval(this.#inputTimer);this.#releaseInput();
    if(this.#role!=='host')return;
    if(this.#inputEpoch>=1000000){this.#inputArmed=false;if(!this.#closed)void this.stop('input_epoch_limit');return;}
    this.#inputEpoch++;this.#inputArmed=enabled===true&&!this.#closed&&this.#transport.pair.input_allowed;this.#policy();
    if(this.#inputArmed)this.#inputTimer=setInterval(()=>{
      if(this.#inputHasFrames&&performance.now()-this.#lastInputAt>250){this.armInput(false);this.dispatchEvent(new Event('input_expired'));}
    },100);
  }
  #policy(){if(this.#role==='host'&&this.#control?.readyState==='open'&&this.#control.bufferedAmount<16384)try{this.#control.send(JSON.stringify({type:'input_policy',session_id:this.#session,epoch:this.#inputEpoch,enabled:this.#inputArmed,gamepad:false}));}catch{}}
  #releaseInput(){this.#inputHasFrames=false;this.dispatchEvent(new Event('release_input'));}
  async #sample(){
    if(this.#statsBusy||this.#closed||!this.#pc)return;this.#statsBusy=true;
    try{const pc=this.#pc,report=await pc.getStats();if(this.#closed||pc!==this.#pc)return;const value=await this.#stats.sample(pc,report);if(this.#closed)return;this.dispatchEvent(new CustomEvent('metrics',{detail:value}));if(this.#closed)return;this.dispatchEvent(new CustomEvent('media_info',{detail:this.#media.sample(report,this.#role)}));if(this.#closed)return;if(this.#role==='client'&&this.#control?.readyState==='open'&&this.#control.bufferedAmount<16384)this.#control.send(JSON.stringify({type:'quality',session_id:this.#session,metrics:value}));}catch{}finally{this.#statsBusy=false;}
  }
  #error(error){if(this.#closed)return;this.dispatchEvent(new CustomEvent('session_error',{detail:{code:error.code??'runtime_crashed',message:error.code?error.message:'The remote media session could not continue.'}}));void this.stop('session_failed');}
  stop(reason='user_stopped',notify=true){
    if(this.#stopPromise)return this.#stopPromise;
    let resolve,reject;this.#stopPromise=new Promise((done,failed)=>{resolve=done;reject=failed;});
    this.#stopOwned(reason,notify).then(resolve,reject);return this.#stopPromise;
  }
  async #stopOwned(reason,notify){
    if(this.#closed)return;this.#closed=true;
    this.#lifetime.abort();this.#captureRequest?.cancel();this.#captureRequest=null;
    this.#quality?.close();this.#quality=null;
    this.#captureSource?.stop();this.#captureSource=null;this.armInput(false);
    clearInterval(this.#timer);clearTimeout(this.#reconnectTimer);clearTimeout(this.#negotiationTimer);this.#reconnectTimer=0;
    const pc=this.#pc;this.#pc=null;
    if(pc){pc.onicecandidate=null;pc.onconnectionstatechange=null;pc.ontrack=null;pc.ondatachannel=null;pc.close();}
    for(const track of this.#stream?.getTracks()??[])track.stop();this.#stream=null;for(const track of this.#remote.getTracks()){track.stop();this.#remote.removeTrack(track);}
    for(const channel of[this.#control,this.#input])if(channel){channel.onmessage=null;channel.onclose=null;channel.onopen=null;}
    this.#control=null;this.#input=null;this.#candidates=[];this.#localCandidates=[];
    this.dispatchEvent(new CustomEvent('state',{detail:{state:'stopping',reason}}));
    const target=this.#session??this.#pending?.session_id;this.#pending=null;
    if(notify&&target&&!this.#transport.closed){let timer;try{await Promise.race([
      this.#transport.send(target,{type:'stop',reason}).catch(()=>{}),new Promise(resolve=>{timer=setTimeout(resolve,2000);})
    ]);}finally{clearTimeout(timer);}}
    let timer;try{await Promise.race([this.#transport.close().catch(()=>{}),new Promise(resolve=>{timer=setTimeout(resolve,2000);})]);}
    finally{clearTimeout(timer);this.dispatchEvent(new CustomEvent('state',{detail:{state:'closed',reason}}));}
  }
}
