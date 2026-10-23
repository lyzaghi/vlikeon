import{validatePortableFilesRequest}from'./md-portable.js';
import{coreMemoryFloor}from'./memory-policy.js';
import{validateHardwareVideo}from'./hardware-video.js';
import{validateThreadPolicy,threadsAvailable}from'./thread-policy.js';
import{validateThreadWasm}from'./thread-wasm.js';
import{UiThreadRelay}from'./thread-relay.js';
export class WebGameRuntime extends EventTarget {
  #worker=null;#pending=new Map();#audio=null;#audioNode=null;#state='created';#canvas=null;#inputInFlight=false;
  #latencyFrames=2;#nominalHz=60;
  #videoGeometry={width:0,height:0};
  #stopTask=null;
  #nativeMedia=null;
  #threads=null;
  #playTask=null;#pauseTask=null;
  #canvasLoss=null;
  get state(){return this.#state;}
  get videoGeometry(){return{...this.#videoGeometry};}
  async unlockAudio(){if(!this.#audio)this.#audio=new AudioContext({latencyHint:'interactive'});await this.#audio.resume();}

  async prepare({moduleUrl,wasmBinary,rom,media=null,loadMedia=null,nativeMedia=null,firmware=null,systemAssets=null,discControlEnabled=false,expectedDiscCount=null,savedFiles=null,portableFiles=null,resultProtocol=0,minimumSystemRamBytes=0,fileBackedContentRequired=false,hardwareVideo=null,threadPolicy=null,signal,canvas,options={},optionPolicy=null,volume=.8,extension='rom',latencyFrames=2,preset='accurate',hapticsEnabled=false,pointerEnabled=false}){
    if(this.#worker||this.#stopTask)throw new Error('Stop the current runtime before preparing another.');
    this.#canvas=canvas;
    if(!Number.isInteger(latencyFrames)||latencyFrames<0||latencyFrames>8||!['accurate','smooth','battery'].includes(preset))throw new Error('Unsupported playback settings.');
    if(![0,1,2].includes(resultProtocol))throw new Error('The emulator result protocol is unsupported.');
    hardwareVideo=validateHardwareVideo(hardwareVideo);
    threadPolicy=validateThreadPolicy(threadPolicy);
    if(threadPolicy){
      if(!threadsAvailable()||resultProtocol!==2||hardwareVideo||portableFiles||savedFiles&&(savedFiles.threads!==true||savedFiles.io!=='pthread_joined_barrier_v1'))throw new Error('This emulator thread combination is not supported.');
      validateThreadWasm(wasmBinary,threadPolicy);
    }
    if(hardwareVideo&&resultProtocol!==2)throw new Error("Hardware video requires the lifecycle completion protocol.");
    minimumSystemRamBytes=coreMemoryFloor({minimum_system_ram_bytes:minimumSystemRamBytes});
    portableFiles=validatePortableFilesRequest(portableFiles);
    if(portableFiles!==null&&!savedFiles)throw new Error('Portable progress requires persistent-file saves.');
    this.#latencyFrames=preset==='smooth'?Math.max(3,latencyFrames):latencyFrames;
    this.#worker=new Worker(new URL('./libretro-worker.js',import.meta.url),{type:'module',name:'OmniPlay emulator'});
    const owner=this.#worker;
    owner.onmessage=event=>{if(this.#worker===owner){try{this.#message(event.data);}catch(error){this.#failed(error?.message||'The emulator message could not be handled.');}}};
    owner.onerror=()=>{if(this.#worker===owner)this.#failed('The isolated emulator worker stopped.');};
    owner.onmessageerror=()=>{if(this.#worker===owner)this.#failed('The emulator returned an invalid message.');};
    const cancelled=()=>{if(signal?.aborted||!this.#worker||['stopping','stopped','failed'].includes(this.#state))throw Object.assign(new Error('Game preparation was cancelled.'),{code:'cancelled'});};
    try{
      if(threadPolicy)this.#threads=new UiThreadRelay(threadPolicy,moduleUrl,owner,message=>this.#failed(message));
      this.#state='preparing';cancelled();
      this.#canvasLoss=event=>{event.preventDefault();if(this.#worker===owner)this.#failed('The video surface was lost. Reopen the game before continuing.');};
      this.#canvas?.addEventListener('contextlost',this.#canvasLoss);
      if(!this.#audio)this.#audio=new AudioContext({latencyHint:'interactive'});
      this.#audio.onstatechange=()=>{
        if(this.#worker!==owner||this.#state!=='running')return;
        if(this.#audio?.state==='closed')this.#failed('The audio device closed. Reopen the game before continuing.');
        else if(this.#audio?.state!=='running')void this.pause().catch(()=>{});
      };
      await this.#audio.audioWorklet.addModule(new URL('./audio-worklet.js',import.meta.url));
      this.#audioNode=new AudioWorkletNode(this.#audio,'omniplay-audio',{outputChannelCount:[2]});
      this.#audioNode.connect(this.#audio.destination);this.#audioNode.port.postMessage({method:'volume',volume});
      this.#audioNode.onprocessorerror=()=>{if(this.#worker===owner)this.#failed('The audio processor stopped. Reopen the game before continuing.');};
      this.#audioNode.port.onmessage=event=>{
        if(this.#worker!==owner)return;
        if(event.data?.event==='audio_ack'){
          if(Number.isSafeInteger(event.data.sequence)&&event.data.sequence>0)owner.postMessage({method:'audio_ack',params:{sequence:event.data.sequence}});
        }else if(event.data?.event==='audio_metrics'){
          this.#audioNode?.port.postMessage({method:'metrics_ack',sequence:event.data.sequence});
          this.dispatchEvent(new CustomEvent('audio_metrics',{detail:event.data}));
        }
      };
      let stageId=null;
      if(nativeMedia){
        if(media||rom!==undefined||typeof nativeMedia.read!=='function'||typeof nativeMedia.close!=='function'||globalThis.crossOriginIsolated!==true||typeof SharedArrayBuffer!=='function')throw new Error('Native game reads require isolated shared memory and one media source.');
        const shared=new SharedArrayBuffer(65536+16),channel={source:nativeMedia,shared,header:new Int32Array(shared,0,4),bytes:new Uint8Array(shared,16),sequence:0,busy:false,closed:false,remove:null};
        this.#nativeMedia=channel;channel.remove=nativeMedia.onFailure?.(()=>this.#failed('The verified native game media lease could not be renewed.'));stageId=nativeMedia.description.set_id;
      }else if(media){
        if(typeof loadMedia!=='function'||rom!==undefined)throw new Error('File-backed media requires a bounded source.');
        const opened=await this.#request('media_begin',media,[],30000);stageId=opened.stage_id;if(typeof stageId!=='string'||opened.chunk_bytes!==48*1024||opened.media_protocol!==1)throw new Error('Media staging protocol is invalid.');cancelled();
        await loadMedia(Object.freeze({start:async name=>{cancelled();return this.#request('media_asset_start',{stage_id:stageId,name});},chunk:async(name,offset,bytes)=>{cancelled();return this.#request('media_chunk',{stage_id:stageId,name,offset,bytes},[bytes],30000);},finish:async name=>{cancelled();return this.#request('media_asset_finish',{stage_id:stageId,name},[],30000);}}));
        cancelled();await this.#request('media_seal',{stage_id:stageId});
      }else if(!(rom instanceof ArrayBuffer))throw new Error('ROM content is missing.');
      cancelled();const params={moduleUrl,options,optionPolicy,resultProtocol,hardwareVideo,threadPolicy,threadWorkerUrl:this.#threads?.workerUrl??null,preset,minimumSystemRamBytes,fileBackedContentRequired:fileBackedContentRequired===true,wasmBinary,extension,hapticsEnabled:hapticsEnabled===true,pointerEnabled:pointerEnabled===true,presentationMaxHz:preset==='battery'?30:0},transfer=[];
      if(stageId)params.media_stage_id=stageId;else{params.rom=rom;transfer.push(rom);}
      params.discControlEnabled=discControlEnabled===true;params.expectedDiscCount=expectedDiscCount;
      if(savedFiles)params.savedFiles=savedFiles;
      if(portableFiles)params.portableFiles=portableFiles;
      if(nativeMedia)params.native_media={description:nativeMedia.description,shared:this.#nativeMedia.shared};
      if(firmware){params.firmware={system:firmware.system,requirements:firmware.requirements,files:firmware.files};for(const file of firmware.files){if(!(file.bytes instanceof ArrayBuffer))throw new Error('Selected firmware content is missing.');transfer.push(file.bytes);}}
      if(systemAssets){if(firmware)throw new Error('System asset and firmware mountpoints collide.');params.systemAssets={protocol:systemAssets.protocol,files:systemAssets.files};for(const file of systemAssets.files){if(!(file.buffer instanceof ArrayBuffer))throw new Error('Bundled system asset content is missing.');transfer.push(file.buffer);}}
      if(wasmBinary instanceof ArrayBuffer)transfer.push(wasmBinary);
      let offscreen2D=false;try{offscreen2D=typeof OffscreenCanvas==='function'&&Boolean(new OffscreenCanvas(1,1).getContext('2d'));}catch{}
      if(hardwareVideo){if(typeof canvas?.transferControlToOffscreen!=='function'||typeof OffscreenCanvas!=='function')throw new Error('This browser cannot transfer the hardware video canvas.');params.canvas=canvas.transferControlToOffscreen();transfer.push(params.canvas);}
      else if(canvas.transferControlToOffscreen&&offscreen2D){params.canvas=canvas.transferControlToOffscreen();transfer.push(params.canvas);}
      cancelled();const info=await this.#request('prepare',params,transfer,30000);cancelled();if(info.systemAssetsProtocol!==(systemAssets?1:0)||info.threadProtocol!==(threadPolicy?1:0)||info.hardwareVideoProtocol!==(hardwareVideo?1:0)||fileBackedContentRequired&&info.fileBackedContent!==true||info.systemMemoryProtocol!==1||!Number.isSafeInteger(info.systemRamBytes)||info.systemRamBytes<minimumSystemRamBytes||info.inputProtocol!==3||info.coreOptionsProtocol!==1||info.libretroResultProtocol!==resultProtocol||info.portableInGameFormat!==(portableFiles?.format??null)||stageId&&info.mediaProtocol!==1||nativeMedia&&info.nativeReadProtocol!==1||firmware&&info.firmwareProtocol!==1||discControlEnabled&&info.discProtocol!==1||savedFiles&&(info.savedFilesProtocol!==1||info.saveFilesSupported!==true)||!savedFiles&&info.saveFilesSupported===true)throw new Error('The installed emulator does not support the required session data.');this.#nominalHz=info.nominalRefreshHz;this.#configureAudio();this.#state='ready';return info;
    }catch(error){await this.stop();throw error;}
  }
  async #play(method){
    if(this.#playTask)return this.#playTask;
    if(this.#pauseTask)await this.#pauseTask;
    if(method==='start'?!['ready','paused'].includes(this.#state):this.#state!=='paused')throw new Error('Game is not ready.');
    const owner=this.#worker,initial=this.#state;
    const operation=(async()=>{
      await this.#audio.resume();
      if(this.#worker!==owner||this.#state!==initial)throw new Error('Playback was cancelled.');
      this.#audioNode?.port.postMessage({method:'clear'});
      try{await this.#request(method);}catch(error){if(this.#worker===owner)this.#failed(error.message);throw error;}
      if(this.#worker!==owner||this.#state!==initial)throw new Error('Playback was cancelled.');
      this.#state='running';
    })();this.#playTask=operation;
    try{return await operation;}finally{if(this.#playTask===operation)this.#playTask=null;}
  }
  async start(){return this.#play('start');}
  async pause(){
    if(this.#pauseTask)return this.#pauseTask;
    if(this.#playTask){try{await this.#playTask;}catch{}}
    if(this.#state!=='running')return;
    const owner=this.#worker;this.#state='pausing';
    const operation=(async()=>{
      this.#audioNode?.port.postMessage({method:'clear'});
      try{await this.#request('pause');await this.#audio?.suspend();}
      catch(error){if(this.#worker===owner)this.#failed(error.message);throw error;}
      finally{this.#audioNode?.port.postMessage({method:'clear'});}
      if(this.#worker===owner&&this.#state==='pausing')this.#state='paused';
    })();this.#pauseTask=operation;
    try{return await operation;}finally{if(this.#pauseTask===operation)this.#pauseTask=null;}
  }
  async resume(){return this.#play('resume');}
  input(ports,analogPorts,hapticsAvailable=[false,false,false,false],pointer={x:0,y:0,pressed:false,inside:false}){if(this.#state==='running'&&!this.#inputInFlight){this.#inputInFlight=true;void this.#request('input',{ports,analogPorts,hapticsAvailable,pointer,pointerGeometry:this.videoGeometry,observed_at_ms:performance.timeOrigin+performance.now()}).then(result=>{const value=result?.videoGeometry;if(!value||!Number.isInteger(value.width)||!Number.isInteger(value.height)||value.width<0||value.height<0||value.width>4096||value.height>4096||Boolean(value.width)!==Boolean(value.height))throw new Error('The emulator returned invalid video geometry.');this.#videoGeometry={width:value.width,height:value.height};}).catch(error=>this.#failed(error.message)).finally(()=>{this.#inputInFlight=false;});}}
  async save(kind){try{const result=await this.#request('save',{kind},[],30000);return result.payload;}catch(error){if(error.code==='runtime_timeout')this.#failed('Save capture timed out. Reopen the game before continuing.');throw error;}}
  async load(kind,payload){
    if(!['ready','paused'].includes(this.#state))throw new Error('Pause the game before restoring saved progress.');
    this.#audioNode?.port.postMessage({method:'clear'});this.#videoGeometry={width:0,height:0};
    try{await this.#request('load',{kind,payload},[payload],30000);}catch(error){if(error.code==='runtime_timeout')this.#failed('Progress restoration timed out. Reopen the game before continuing.');throw error;}
    finally{this.#audioNode?.port.postMessage({method:'clear'});}
  }
  async discStatus(){return this.#request('disc_status');}
  async selectDisc(index){if(!['ready','paused'].includes(this.#state))throw new Error('Pause the game before changing discs.');return this.#request('disc_select',{index},[],30000);}
  async collectMeasurement(enabled){await this.#request('measurement',{enabled:enabled===true});}
  async stop(){
    if(this.#stopTask)return this.#stopTask;
    const operation=(async()=>{
      this.#state='stopping';this.#audioNode?.port.postMessage({method:'clear'});const native=this.#closeNative(),worker=this.#worker;
      if(worker){try{await this.#request('stop',{},[],2000);}catch{}this.#threads?.close();this.#threads=null;worker.terminate();this.#worker=null;}
      this.#threads?.close();this.#threads=null;
      if(this.#canvasLoss)this.#canvas?.removeEventListener('contextlost',this.#canvasLoss);this.#canvasLoss=null;
      for(const item of this.#pending.values()){clearTimeout(item.timer);item.reject(new Error('Runtime stopped.'));}this.#pending.clear();
      if(this.#audioNode){this.#audioNode.onprocessorerror=null;this.#audioNode.port.onmessage=null;this.#audioNode.port.close();this.#audioNode.disconnect();}
      if(this.#audio)this.#audio.onstatechange=null;
      try{await this.#audio?.close();await native;}finally{this.#audio=null;this.#audioNode=null;this.#state='stopped';this.#inputInFlight=false;this.#videoGeometry={width:0,height:0};}
    })();this.#stopTask=operation;
    try{return await operation;}finally{if(this.#stopTask===operation)this.#stopTask=null;}
  }
  #request(method,params={},transfer=[],timeout=5000){
    if(!this.#worker)return Promise.reject(new Error('Runtime is unavailable.'));
    if(this.#pending.size>=32)return Promise.reject(new Error('Runtime command queue is full.'));
    const id=crypto.randomUUID();return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.#pending.delete(id);if(this.#threads)this.#failed('The threaded emulator exceeded its operation deadline.');reject(Object.assign(new Error('Runtime operation timed out.'),{code:'runtime_timeout'}));},timeout);this.#pending.set(id,{resolve,reject,timer});try{this.#worker.postMessage({id,method,params},transfer);}catch(error){clearTimeout(timer);this.#pending.delete(id);reject(error);}});
  }
  #message(message){
    if(!message||typeof message!=='object')throw new Error('The emulator returned an invalid message.');
    if(message.event){
      if(message.event==='pthread'){if(this.#threads)this.#threads.handle(message.data);else this.#failed('The emulator requested undeclared threads.');return;}
      if(message.event==='native_media_read'){void this.#readNative(message.data);return;}
      if(message.event==='haptics'){
        const packet=message.data;if(!packet||!Number.isSafeInteger(packet.sequence)||packet.sequence<1)return;
        this.#worker?.postMessage({method:'haptics_ack',params:{sequence:packet.sequence}});
        const age=Number.isFinite(packet.published_at_ms)?performance.timeOrigin+performance.now()-packet.published_at_ms:NaN;
        const values=this.#state==='running'&&Number.isFinite(age)&&age>=-50&&age<=250?packet.values:Array.from({length:4},()=>({strong:0,weak:0}));
        this.dispatchEvent(new CustomEvent('haptics',{detail:{values,age_ms:Number.isFinite(age)?Math.max(0,age):251}}));
      }
      else if(message.event==='audio'){
        const packet=message.data;
        if(!packet||!Number.isSafeInteger(packet.sequence)||packet.sequence<1||!(packet.samples instanceof ArrayBuffer)||
          packet.samples.byteLength===0||packet.samples.byteLength>16384*2*4||packet.samples.byteLength%8||
          !Number.isFinite(packet.sampleRate)||packet.sampleRate<8000||packet.sampleRate>192000){this.#failed('The emulator returned invalid audio.');return;}
        // Paused/preparing/failed sessions must not feed audio produced by
        // load/init callbacks into a suspended worklet for replay on resume.
        if(this.#state!=='running'||!this.#audioNode){this.#worker?.postMessage({method:'audio_ack',params:{sequence:packet.sequence}});return;}
        this.#audioNode.port.postMessage({method:'samples',...packet},[packet.samples]);
      }
      else if(message.event==='video'){
        const packet=message.data;let presented=false;
        try{
          if(!packet||!Number.isSafeInteger(packet.sequence)||packet.sequence<1||!Number.isInteger(packet.width)||!Number.isInteger(packet.height)||packet.width<1||packet.height<1||packet.width>4096||packet.height>4096||!(packet.rgba instanceof ArrayBuffer)||packet.rgba.byteLength!==packet.width*packet.height*4)throw new Error('The emulator returned invalid video.');
          if(this.#canvas&&this.#state==='running'){
            const{rgba,width,height}=packet;if(this.#canvas.width!==width)this.#canvas.width=width;if(this.#canvas.height!==height)this.#canvas.height=height;
            const context=this.#canvas.getContext('2d',{alpha:false});if(!context)throw new Error('The video surface is unavailable.');
            context.putImageData(new ImageData(new Uint8ClampedArray(rgba),width,height),0,0);
            presented=true;
          }
        }catch(error){this.#failed(error.message);}
        finally{if(Number.isSafeInteger(packet?.sequence))this.#worker?.postMessage({method:'video_ack',params:{sequence:packet.sequence,presented}});}
      }
      else if(message.event==='runtime_error')this.#failed(message.data.message);
      else if(message.event==='av_info'){
        const value=message.data;
        if(!value||!Number.isFinite(value.fps)||value.fps<20||value.fps>240||!Number.isFinite(value.sampleRate)||value.sampleRate<8000||value.sampleRate>192000||!Number.isSafeInteger(value.sequence)||value.sequence<1){this.#failed('The emulator returned invalid AV timing.');return;}
        this.#worker?.postMessage({method:'av_ack',params:{sequence:value.sequence}});
        this.#nominalHz=value.fps;this.#configureAudio();this.dispatchEvent(new CustomEvent(message.event,{detail:value}));
      }
      else if(message.event==='metrics'){if(Number.isSafeInteger(message.data?.sequence))this.#worker?.postMessage({method:'metrics_ack',params:{sequence:message.data.sequence}});this.dispatchEvent(new CustomEvent(message.event,{detail:message.data}));}
      else this.dispatchEvent(new CustomEvent(message.event,{detail:message.data}));
      return;
    }
    const item=this.#pending.get(message.id);if(!item)return;clearTimeout(item.timer);this.#pending.delete(message.id);message.status==='ok'?item.resolve(message.result):item.reject(Object.assign(new Error(message.error?.message||'Runtime operation failed.'),{code:message.error?.code}));
  }
  #configureAudio(){this.#audioNode?.port.postMessage({method:'configure',latencyFrames:this.#latencyFrames,nominalHz:this.#nominalHz});}
  async #readNative(request){
    const channel=this.#nativeMedia;if(!channel||channel.closed)return;
    const valid=request&&Number.isInteger(request.sequence)&&request.sequence===channel.sequence+1&&request.sequence>0&&request.sequence<=0x7fffffff&&request.set_id===channel.source.description.set_id&&Number.isInteger(request.asset_index)&&Number.isSafeInteger(request.offset)&&Number.isInteger(request.limit)&&request.limit>0&&request.limit<=65536;
    if(!valid||channel.busy||Atomics.load(channel.header,0)!==1||Atomics.load(channel.header,1)!==request.sequence){this.#failed('The native media request channel is invalid.');return;}
    channel.busy=true;channel.sequence=request.sequence;
    try{
      const bytes=await channel.source.read(request);
      if(channel.closed||this.#nativeMedia!==channel)return;
      if(Atomics.load(channel.header,0)!==1||Atomics.load(channel.header,1)!==request.sequence){this.#failed('The native game read expired before its reply arrived.');return;}
      if(!(bytes instanceof Uint8Array)||bytes.length!==request.limit)throw new Error('The native media response has an invalid size.');
      channel.bytes.set(bytes);Atomics.store(channel.header,2,bytes.length);Atomics.store(channel.header,0,2);Atomics.notify(channel.header,0);
    }catch{if(!channel.closed){Atomics.store(channel.header,0,3);Atomics.notify(channel.header,0);this.#failed('The verified game asset could not be read. Reopen the game to verify its media again.');}}
    finally{channel.busy=false;}
  }
  #closeNative(){
    const channel=this.#nativeMedia;this.#nativeMedia=null;if(!channel)return Promise.resolve();channel.closed=true;channel.remove?.();Atomics.store(channel.header,0,4);Atomics.notify(channel.header,0);return Promise.resolve(channel.source.close()).catch(()=>{});
  }
  #failed(message){
    if(this.#state==='failed'||this.#state==='stopped')return;
    this.#state='failed';const worker=this.#worker;this.#worker=null;this.#threads?.close();this.#threads=null;worker?.terminate();
    void this.#closeNative();
    for(const item of this.#pending.values()){clearTimeout(item.timer);item.reject(Object.assign(new Error(message),{code:'runtime_crashed'}));}this.#pending.clear();
    this.#audioNode?.port.postMessage({method:'clear'});this.#audio?.suspend().catch(()=>{});
    this.dispatchEvent(new CustomEvent('runtime_error',{detail:{message}}));
  }
}
