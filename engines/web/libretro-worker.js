// Dedicated worker: emulator memory, ROM parsing and callbacks never run in the UI.
import{MediaStage}from'./media-stage.js';
import{NativeMedia}from'./native-media.js';
import{mountFirmware}from'./firmware.js';
import{DiscControl,discEnvelope}from'./disc-control.js';
import{SavedFileSession}from'./saved-file-session.js';
import{resolveCoreOptions}from'./core-options.js';
import{LibretroCalls}from'./libretro-calls.js';
import{coreMemoryFloor,observeSystemMemory}from'./memory-policy.js';
import{HardwareVideo,validateHardwareVideo}from'./hardware-video.js';
import{validateThreadPolicy,threadWorkerUrl,threadsAvailable}from'./thread-policy.js';
import{validateThreadWasm}from'./thread-wasm.js';
import{RemoteThreadPool,blockNestedWorkers}from'./thread-relay.js';
import{ThreadCallbacks}from'./thread-callbacks.js';
import{mountCoreAssets}from'./core-assets.js';
const MAX_ROM=256*1024*1024,MAX_SAVE=256*1024*1024,MAX_PIXELS=4096*4096;
let core=null,running=false,timer=null,deadline=0,framePeriod=1000/60,pixelFormat=0,canvas=null,context=null;
let initialized=false,loaded=false;
let terminal=false;
let mediaStage=null;
let unmountFirmware=null;
let discControl=null;
let savedFileSession=null,frontendCallbacks=null;
let guestCalls=null;
let minimumSystemRam=0,observedSystemRam=0;
let hardwareVideo=null,hardwarePolicy=null,playbackPreset='accurate';
let threadPool=null,threadCallbacks=null;
let activeGameInfo=0,romPointer=0,pathPointer=0,romPath='',callbackPointers=[],persistentStrings=[];
const stringCache=new Map();
let sampleRate=48000,frames=0,droppedFrames=0,lastMetrics=0,frameTimes=[],previousFrameAt=0;
let audioSequence=0,audioPending=new Set(),sampleBatch=new Float32Array(2048),sampleCount=0,audioDroppedFrames=0;
let videoSequence=0,videoPending=0;
let metricsSequence=0,metricsPending=0,avSequence=0,avPending=0,avQueued=null;
function publishAv(value){
  if(avPending){avQueued=value;return;}
  if(avSequence>=Number.MAX_SAFE_INTEGER)fail('AV sequence exhausted.');
  avPending=++avSequence;emit('av_info',{...value,sequence:avPending});
}
function sendAudio(samples){
  if(!running)return;
  if(audioPending.size>=4){audioDroppedFrames+=samples.length/2;return;}
  if(audioSequence>=Number.MAX_SAFE_INTEGER)fail('Audio sequence exhausted.');
  const sequence=++audioSequence;audioPending.add(sequence);
  emit('audio',{sequence,samples: samples.buffer,sampleRate},[samples.buffer]);
}
function flushSamples(){if(!sampleCount)return;const samples=sampleBatch.slice(0,sampleCount);sampleCount=0;sendAudio(samples);}
function audioSample(left,right){
  if(!running)return;
  sampleBatch[sampleCount++]=left/32768;sampleBatch[sampleCount++]=right/32768;
  if(sampleCount===sampleBatch.length)flushSamples();
}
let input=Array.from({length:4},()=>new Int16Array(16));
let analogInput=Array.from({length:4},()=>new Int16Array(4)),lastInputAt=-1;
let pointerEnabled=false,pointerInput={x:0,y:0,pressed:false,inside:false},videoWidth=0,videoHeight=0;
let displayAspect=0;
function displaySize(width,height){
  const ratio=displayAspect>0?displayAspect:width/height;
  let w=Math.max(1,Math.round(height*ratio)),h=height;
  const scale=Math.min(1,(hardwarePolicy?.maximum_width??4096)/w,(hardwarePolicy?.maximum_height??4096)/h);
  return{width:Math.max(1,Math.floor(w*scale)),height:Math.max(1,Math.floor(h*scale))};
}
let presentationMaxHz=0,nextPresentationAt=0,framesEmulated=0,coreRunTimes=[];
let collectMeasurement=false;
const coreVariables=new Map(),variableDefinitions=new Map();
let hapticsEnabled=false,rumblePointer=0,hapticsAvailable=[false,false,false,false],lastHapticsAt=0;
let rumble=Array.from({length:4},()=>({strong:0,weak:0})),sentRumble=Array.from({length:4},()=>({strong:0,weak:0}));
let hapticsSequence=0,hapticsPending=0,hapticsQueued=null;
function publishRumble(values,at=performance.timeOrigin+performance.now()){
  if(hapticsPending){hapticsQueued={values,at};return;}
  hapticsPending=++hapticsSequence;emit('haptics',{sequence:hapticsPending,published_at_ms:at,values});
}
function setRumble(port,effect,strength){if(!hapticsEnabled||!running||!hapticsAvailable[port]||!Number.isInteger(port)||port<0||port>=4||![0,1].includes(effect)||!Number.isInteger(strength)||strength<0||strength>65535)return 0;rumble[port][effect===0?'strong':'weak']=strength;return 1;}
function clearRumble(){rumble=Array.from({length:4},()=>({strong:0,weak:0}));sentRumble=Array.from({length:4},()=>({strong:0,weak:0}));publishRumble(sentRumble);}
function flushRumble(now){if(!hapticsEnabled||now-lastHapticsAt<50)return;const values=rumble.map((item,port)=>running&&hapticsAvailable[port]?{strong:item.strong,weak:item.weak}:{strong:0,weak:0});if(values.some((v,p)=>v.strong||v.weak||sentRumble[p].strong||sentRumble[p].weak)){publishRumble(values);sentRumble=values;lastHapticsAt=now;}}
function clearInput(){input=Array.from({length:4},()=>new Int16Array(16));analogInput=Array.from({length:4},()=>new Int16Array(4));pointerInput={x:0,y:0,pressed:false,inside:false};lastInputAt=-1;hapticsAvailable=[false,false,false,false];clearRumble();}
function inputState(port,device,index,id){
  if(!running||!Number.isInteger(port)||port<0||port>=4||!Number.isInteger(index)||!Number.isInteger(id)||index<0||id<0)return 0;
  device&=0xff;
  if(device===1&&index===0){if(id===256){let mask=0;for(let bit=0;bit<16;bit++)if(input[port][bit])mask|=1<<bit;return(mask<<16)>>16;}return id<16?input[port][id]:0;}
  if(device===6&&pointerEnabled&&port===0&&index===0){if(id===0)return pointerInput.x;if(id===1)return pointerInput.y;if(id===2||id===3)return pointerInput.pressed?1:0;if(id===15)return pointerInput.inside?0:1;return 0;}
  return device===5&&index<2&&id<2?analogInput[port][index*2+id]:0;
}


function emit(event,data,transfer=[]){self.postMessage({event,data},transfer);}
function fail(message){const error=new Error(message);error.code='runtime_unavailable';throw error;}
function allocateString(value){if(stringCache.has(value))return stringCache.get(value);const size=core.lengthBytesUTF8(value)+1,pointer=core._malloc(size);if(!pointer)fail('Emulator memory allocation failed.');core.stringToUTF8(value,pointer,size);persistentStrings.push(pointer);stringCache.set(value,pointer);return pointer;}
function readString(pointer){return pointer?core.UTF8ToString(pointer):'';}
function ensureDirectory(path){try{core.FS.mkdir(path);}catch(error){if(!core.FS.analyzePath(path).exists)throw error;}}

function environment(command,data){
  if(command&0x20000)return 0;
  const code=command&0xffff;
  switch(code){
    case 81: // GET_TARGET_SAMPLE_RATE: avoid a core's multi-MHz fallback.
      if(!Number.isInteger(data)||data<=0||data%4||data>core.HEAPU8.length-4)return 0;
      core.HEAPU32[data>>>2]=48000;return 1;
    case 3: core.HEAPU8[data]=1;return 1; // GET_CAN_DUPE
    case 9: core.HEAPU32[data>>>2]=allocateString('/system');return 1;
    case 10:{const format=core.HEAPU32[data>>>2];if(format>2)return 0;pixelFormat=format;return 1;}
    case 11:return 1; // Input descriptors are optional frontend hints.
    case 13:case 57:case 58:return discControl?.environment(code,data)??0;
    case 14:return hardwareVideo?.register(data)??0;
    case 15:{const key=readString(core.HEAPU32[data>>>2]);const value=coreVariables.get(key);if(value===undefined){core.HEAPU32[(data+4)>>>2]=0;return 0;}core.HEAPU32[(data+4)>>>2]=allocateString(value);return 1;}
    case 16:{
      for(let cursor=data,count=0;count<512;cursor+=8,count++){
        const keyPointer=core.HEAPU32[cursor>>>2];if(!keyPointer)break;
        const key=readString(keyPointer),definition=readString(core.HEAPU32[(cursor+4)>>>2]);
        const choices=definition.split(';')[1]?.trim().split('|')||[];
        variableDefinitions.set(key,choices);if(!coreVariables.has(key)&&choices.length)coreVariables.set(key,choices[0]);
      }return 1;
    }
    case 17:core.HEAPU8[data]=0;return 1;
    case 23:if(!hapticsEnabled||!rumblePointer||!Number.isInteger(data)||data<=0||data%4||data+4>core.HEAPU8.length)return 0;core.HEAPU32[data>>>2]=rumblePointer;return 1;
    case 24:if(!Number.isInteger(data)||data<=0||data%4||data+8>core.HEAPU8.length)return 0;core.HEAPU32[data>>>2]=(1<<1)|(1<<5)|(pointerEnabled?(1<<6):0);core.HEAPU32[(data+4)>>>2]=0;return 1;
    case 27:if(typeof core._om_web_log_interface!=='function'||!Number.isInteger(data)||data<=0||data%4||data+4>core.HEAPU8.length)return 0;return core._om_web_log_interface(data)===1?1:0;
    case 18:throw Object.assign(new Error('The emulator requested shutdown.'),{unrecoverable:true});
    case 30:core.HEAPU32[data>>>2]=allocateString('/assets');return 1;
    case 31:core.HEAPU32[data>>>2]=allocateString('/saves');return 1;
    case 32:readAvInfo(data);return 1;
    case 37:readGeometry(data);return 1;
    case 39:core.HEAPU32[data>>>2]=0;return 1;
    case 47:core.HEAPU32[data>>>2]=3;return 1;
    case 49:core.HEAPU8[data]=0;return 1;
    case 50:core.HEAPF32[data>>>2]=1000/framePeriod;return 1;
    case 51:return 1; // JOYPAD_MASK is a signed 16-bit view; data may be null.
    case 52:core.HEAPU32[data>>>2]=1;return 1;
    case 53:{ // Core options v1: key, description, info, 128 value/label pairs, default.
      for(let cursor=data,count=0;count<512;cursor+=1040,count++){
        const keyPointer=core.HEAPU32[cursor>>>2];if(!keyPointer)break;
        const key=readString(keyPointer),choices=[];
        for(let index=0;index<128;index++){const pointer=core.HEAPU32[(cursor+12+index*8)>>>2];if(!pointer)break;choices.push(readString(pointer));}
        const defaultValue=readString(core.HEAPU32[(cursor+1036)>>>2])||choices[0];
        variableDefinitions.set(key,choices);if(!coreVariables.has(key)&&defaultValue)coreVariables.set(key,defaultValue);
      }return 1;
    }
    case 61:core.HEAPU32[data>>>2]=4;return 1;
    case 74:core.HEAPU8[data]=0;return 1;
    case 87:emit('serialization_quirks',{low:core.HEAPU32[data>>>2],high:core.HEAPU32[(data+4)>>>2]});return 1;
    default:return 0;
  }
}

function video(data,width,height,pitch){
  if(!data||!running)return;
  const hardwareFrame=(data>>>0)===0xffffffff;
  if(Boolean(hardwareVideo)!==hardwareFrame)fail('The emulator returned a frame for an undeclared video backend.');
  if(!Number.isInteger(width)||!Number.isInteger(height)||width<1||height<1||width*height>MAX_PIXELS||width>4096||height>4096)fail('Runtime video dimensions exceed policy.');
  if(hardwarePolicy&&(width>hardwarePolicy.maximum_width||height>hardwarePolicy.maximum_height))fail('Hardware video dimensions exceed the declared target.');
  const at=performance.now();
  if(presentationMaxHz&&at+1<nextPresentationAt)return;
  if(presentationMaxHz){nextPresentationAt=Math.max(nextPresentationAt+1000/presentationMaxHz,at);}
  const display=displaySize(width,height),outputWidth=display.width,outputHeight=display.height;
  if(hardwareVideo){if(canvas.width!==outputWidth)canvas.width=outputWidth;if(canvas.height!==outputHeight)canvas.height=outputHeight;hardwareVideo.present(width,height);if(videoWidth!==outputWidth||videoHeight!==outputHeight)pointerInput={x:0,y:0,pressed:false,inside:false};videoWidth=outputWidth;videoHeight=outputHeight;frames++;return;}
  const bytesPerPixel=pixelFormat===1?4:2;
  if(!Number.isInteger(data)||data<=0||!Number.isInteger(pitch)||pitch<width*bytesPerPixel||pitch>64*1024*1024/height||data>core.HEAPU8.length-(height-1)*pitch-width*bytesPerPixel)fail('Runtime video buffer is outside its memory.');
  if(!context&&videoPending){droppedFrames++;return;} // One fallback frame in flight; no growing UI queue.
  const output=new Uint8ClampedArray(outputWidth*outputHeight*4),sourceView=new DataView(core.HEAPU8.buffer);
  for(let y=0;y<outputHeight;y++)for(let x=0;x<outputWidth;x++){
    const source=data+Math.floor(y*height/outputHeight)*pitch+Math.floor(x*width/outputWidth)*bytesPerPixel,offset=(y*outputWidth+x)*4;
    if(pixelFormat===1){const value=sourceView.getUint32(source,true);output[offset]=(value>>>16)&255;output[offset+1]=(value>>>8)&255;output[offset+2]=value&255;}
    else{const value=sourceView.getUint16(source,true);if(pixelFormat===2){output[offset]=Math.round(((value>>>11)&31)*255/31);output[offset+1]=Math.round(((value>>>5)&63)*255/63);output[offset+2]=Math.round((value&31)*255/31);}else{output[offset]=Math.round(((value>>>10)&31)*255/31);output[offset+1]=Math.round(((value>>>5)&31)*255/31);output[offset+2]=Math.round((value&31)*255/31);}}
    output[offset+3]=255;
  }
  if(context){if(canvas.width!==outputWidth)canvas.width=outputWidth;if(canvas.height!==outputHeight)canvas.height=outputHeight;context.putImageData(new ImageData(output,outputWidth,outputHeight),0,0);}
  else if(!videoPending){if(videoSequence>=Number.MAX_SAFE_INTEGER)fail('Video sequence exhausted.');videoPending=++videoSequence;emit('video',{sequence:videoPending,width:outputWidth,height:outputHeight,rgba:output.buffer},[output.buffer]);}
  if(videoWidth!==outputWidth||videoHeight!==outputHeight)pointerInput={x:0,y:0,pressed:false,inside:false};videoWidth=outputWidth;videoHeight=outputHeight;if(context)frames++;
}

function audioBatch(data,count){
  if(!Number.isInteger(count)||count<0||count>16384)fail('Runtime audio batch exceeds policy.');
  if(!count)return 0;
  if(!Number.isInteger(data)||data<=0||data%2||data>core.HEAPU8.length-count*4)fail('Runtime audio buffer exceeds policy.');
  if(!running)return count;
  flushSamples();
  if(audioPending.size>=4){audioDroppedFrames+=count;return count;} // Never block guest retirement.
  const output=new Float32Array(count*2),offset=data>>>1;
  for(let index=0;index<output.length;index++)output[index]=core.HEAP16[offset+index]/32768;
  sendAudio(output);
  return count;
}

function readGeometry(pointer){
  if(!Number.isInteger(pointer)||pointer<=0||pointer%4||pointer>core.HEAPU8.length-20)fail('Runtime geometry storage is invalid.');
  const width=core.HEAPU32[pointer>>>2],height=core.HEAPU32[(pointer+4)>>>2],aspect=core.HEAPF32[(pointer+16)>>>2];
  if(width<1||height<1||width>4096||height>4096||!Number.isFinite(aspect)||aspect<0||aspect>10)fail('Runtime video geometry is invalid.');
  displayAspect=aspect>.1?aspect:0;
}
function readAvInfo(pointer){
  if(!Number.isInteger(pointer)||pointer<=0||pointer%8||pointer>core.HEAPU8.length-40)fail('Runtime AV storage is invalid.');
  readGeometry(pointer);
  // retro_system_av_info: geometry occupies 20 bytes, then ABI alignment
  // places the two doubles in retro_system_timing at offsets 24 and 32.
  const fps=core.HEAPF64[(pointer+24)>>>3],rate=core.HEAPF64[(pointer+32)>>>3];
  if(!Number.isFinite(fps)||fps<20||fps>240||!Number.isFinite(rate)||rate<8000||rate>192000)fail('Runtime audio/video timing is invalid.');
  if(sampleRate!==rate){flushSamples();sampleCount=0;}
  framePeriod=1000/fps;sampleRate=rate;publishAv({fps,sampleRate});
}

async function prepare(params){
  if(core)fail('Stop the current core before preparing another.');
  minimumSystemRam=coreMemoryFloor({minimum_system_ram_bytes:params.minimumSystemRamBytes??0});observedSystemRam=0;
  hardwarePolicy=validateHardwareVideo(params.hardwareVideo??null);playbackPreset=params.preset==='battery'?'battery':'accurate';
  if(hardwarePolicy&&(typeof OffscreenCanvas!=='function'||!(params.canvas instanceof OffscreenCanvas)||params.resultProtocol!==2))fail('Hardware video requires its transferred canvas and lifecycle completion protocol.');
  if(params.native_media){if(mediaStage)fail('Media already has an active source.');mediaStage=await NativeMedia.create(params.native_media.description,params.native_media.shared);}
  const staged=typeof params.media_stage_id==='string';
  if(staged){if(!mediaStage||mediaStage.id!==params.media_stage_id||params.rom!==undefined)fail('The staged media capability is invalid.');}
  else if(mediaStage||!(params.rom instanceof ArrayBuffer)||params.rom.byteLength===0||params.rom.byteLength>MAX_ROM)fail('ROM size exceeds the web runtime limit.');
  presentationMaxHz=params.presentationMaxHz===30?30:0;nextPresentationAt=0;
  const moduleUrl=new URL(params.moduleUrl,self.location.href);
  if(moduleUrl.origin!==self.location.origin||!moduleUrl.pathname.includes('/cores/'))fail('Core module must be bundled with this application.');
  const threadPolicy=validateThreadPolicy(params.threadPolicy??null);let threadMemory=null;
  if(threadPolicy){
    if(!threadsAvailable()||params.resultProtocol!==2||hardwarePolicy||params.portableFiles||params.savedFiles&&(params.savedFiles.threads!==true||params.savedFiles.io!=='pthread_joined_barrier_v1')||params.threadWorkerUrl!==threadWorkerUrl(threadPolicy,moduleUrl.href))fail('The emulator thread contract is unsupported.');
    threadMemory=validateThreadWasm(params.wasmBinary,threadPolicy);
    threadPool=new RemoteThreadPool(threadPolicy,params.threadWorkerUrl,(data,transfer=[])=>emit('pthread',data,transfer),()=>{terminal=true;running=false;clearTimeout(timer);emit('runtime_error',{code:'runtime_crashed',message:'The emulator thread relay failed.'});});
  }else{if(params.threadWorkerUrl!=null)fail('Thread resources require their explicit execution policy.');blockNestedWorkers();}
  const imported=await import(moduleUrl.href);
  if(typeof imported.default!=='function')fail('Core module is not an Emscripten factory.');
  core=await imported.default({noInitialRun:true,...(hardwarePolicy?{canvas:params.canvas}:{}),...(threadPolicy?{mainScriptUrlOrBlob:moduleUrl.href}:{}),wasmBinary:params.wasmBinary,locateFile:file=>new URL(file,moduleUrl).href,print:()=>{},printErr:()=>{}});
  guestCalls=new LibretroCalls(core,params.resultProtocol??0,()=>threadCallbacks?.check());
  if(core._retro_api_version()!==1)fail('Core libretro API version is unsupported.');
  discControl=new DiscControl(core,{enabled:params.discControlEnabled===true,expectedCount:params.expectedDiscCount??null});
  for(const path of ['/system','/assets','/saves','/game','/scratch'])ensureDirectory(path);
  if(params.portableFiles!=null&&!params.savedFiles)fail('Portable progress requires persistent-file saves.');
  if(params.savedFiles&&params.savedFiles.threads!==Boolean(threadPolicy))fail('Persistent-file I/O must match the actual thread policy.');
  if(params.savedFiles)savedFileSession=new SavedFileSession(core,params.savedFiles,()=>guestCalls.flushSaveIo(),params.portableFiles??null);
  if(params.firmware)unmountFirmware=await mountFirmware(core,params.firmware);
  if(params.systemAssets){if(params.firmware)fail('System assets and user firmware cannot share a mountpoint.');unmountFirmware=await mountCoreAssets(core,params.systemAssets);}
  if(params.canvas){canvas=params.canvas;if(!hardwarePolicy){
    context=canvas.getContext('2d',{alpha:false,desynchronized:true});if(!context)fail('Canvas renderer could not be initialized.');
    canvas.addEventListener('contextlost',event=>{event.preventDefault();if(terminal)return;terminal=true;running=false;clearTimeout(timer);sampleCount=0;clearInput();emit('runtime_error',{code:'runtime_crashed',message:'The video surface was lost. Reopen the game before continuing.'});});
  }}
  if(hardwarePolicy)hardwareVideo=new HardwareVideo(core,hardwarePolicy,canvas,()=>{terminal=true;running=false;clearTimeout(timer);timer=null;clearInput();emit('runtime_error',{code:'runtime_crashed',message:'The graphics context was lost. Reopen the game before continuing.'});},playbackPreset);
  const effectiveOptions=resolveCoreOptions(params.optionPolicy??{schema_version:1,definitions:[]},{schema_version:1,values:params.options??{}});
  if(Object.hasOwn(effectiveOptions,'beetle_psx_enable_memcard1')&&!savedFileSession)fail('A selectable second card requires persistent-file saves.');
  for(const[key,value]of Object.entries(effectiveOptions))coreVariables.set(key,value);
  function callback(fn,signature){const pointer=core.addFunction(fn,signature);callbackPointers.push(pointer);return pointer;}
  hapticsEnabled=params.hapticsEnabled===true;pointerEnabled=params.pointerEnabled===true;
  const callbacks={environment,video_refresh:video,audio_sample:audioSample,audio_sample_batch:audioBatch,input_poll:()=>{},input_state:inputState};
  if(threadPolicy){
    threadCallbacks=new ThreadCallbacks(core,threadPolicy,{...callbacks,rumble:setRumble},threadMemory.minimum_bytes);
    frontendCallbacks=Object.fromEntries(Object.keys(callbacks).map(name=>[name,threadCallbacks.pointers[name]]));rumblePointer=hapticsEnabled?threadCallbacks.pointers.rumble:0;
  }else{
    rumblePointer=hapticsEnabled?callback(setRumble,'iiii'):0;
    const signatures={environment:'iii',video_refresh:'viiii',audio_sample:'vii',audio_sample_batch:'iii',input_poll:'v',input_state:'iiiii'};
    frontendCallbacks=Object.fromEntries(Object.entries(callbacks).map(([name,fn])=>[name,callback(fn,signatures[name])]));
  }
  initializeGuest();
  const systemInfo=core._malloc(16);if(!systemInfo)fail('Core system information allocation failed.');
  core._retro_get_system_info(systemInfo);
  const name=readString(core.HEAPU32[systemInfo>>>2]),version=readString(core.HEAPU32[(systemInfo+4)>>>2]),fullpath=Boolean(core.HEAPU8[systemInfo+12]);
  core._free(systemInfo);
  if(params.fileBackedContentRequired===true&&(!staged||!fullpath))fail('The installed emulator did not accept required file-backed content.');
  const extension=['nes','gb','gbc','sfc','gba','gen','bin','nds','pbp','iso','chd','cue','m3u','cso','z64','v64','n64'].includes(params.extension)?params.extension:'rom';
  let memory=null;
  if(staged){romPath=mediaStage.mount(core,params.media_stage_id);if(!fullpath){if(mediaStage.media.assets.length!==1)fail('This core does not accept file-backed multi-file content.');memory=mediaStage.readEntry();}}
  else{romPath=`/game/content.${extension}`;if(fullpath)core.FS.writeFile(romPath,new Uint8Array(params.rom));else memory=new Uint8Array(params.rom);}
  pathPointer=allocateString(romPath);
  if(memory){romPointer=core._malloc(memory.byteLength);if(!romPointer)fail('ROM memory allocation failed.');core.HEAPU8.set(memory,romPointer);}
  activeGameInfo=core._malloc(16);if(!activeGameInfo)fail('Game information allocation failed.');
  core.HEAPU32[activeGameInfo>>>2]=pathPointer;core.HEAPU32[(activeGameInfo+4)>>>2]=romPointer;core.HEAPU32[(activeGameInfo+8)>>>2]=memory?.byteLength??0;core.HEAPU32[(activeGameInfo+12)>>>2]=0;
  const discs=loadPreparedGame();
  if(savedFileSession){savedFileSession.flush();const rollbackBytes=guestCalls.serializeSize();if(!rollbackBytes||rollbackBytes>MAX_SAVE-(discControl.enabled?24:0))fail('Persistent-file saves require an available bounded rollback snapshot.');}
  frames=0;framesEmulated=0;droppedFrames=0;audioDroppedFrames=0;frameTimes=[];coreRunTimes=[];lastMetrics=performance.now();previousFrameAt=0;
  const saveRamBytes=core._retro_get_memory_size(0),saveRtcBytes=core._retro_get_memory_size(1),inGameSupported=Boolean(saveRamBytes||saveRtcBytes||savedFileSession?.inGameAvailable);
  threadCallbacks?.check();
  return{name,version,systemAssetsProtocol:params.systemAssets?1:0,threadProtocol:threadCallbacks?1:0,hardwareVideoProtocol:hardwareVideo?1:0,inputProtocol:3,coreOptionsProtocol:1,libretroResultProtocol:guestCalls.protocol,systemMemoryProtocol:1,systemRamBytes:observedSystemRam,fileBackedContent:staged&&fullpath,mediaProtocol:1,nativeReadProtocol:1,firmwareProtocol:1,discProtocol:discControl.enabled?1:0,savedFilesProtocol:savedFileSession?1:0,saveFilesSupported:Boolean(savedFileSession),inGameSupported,portableInGameFormat:savedFileSession?.portableFormat??null,discs,nominalRefreshHz:1000/framePeriod,sampleRate,saveRamBytes,saveRtcBytes,saveStateBytes:guestCalls.serializeSize(),variables:Object.fromEntries(variableDefinitions)};
}

function initializeGuest(){
  if(!guestCalls?.healthy)throw Object.assign(new Error('The emulator call state is no longer usable.'),{unrecoverable:true});
  discControl.clear();pixelFormat=0;
  for(const[name,pointer]of Object.entries(frontendCallbacks))core['_retro_set_'+name](pointer);
  initialized=true;guestCalls.init();
}
function loadPreparedGame(){
  if(!guestCalls.loadGame(activeGameInfo))fail('The selected core could not load this game.');loaded=true;
  hardwareVideo?.activate();
  observedSystemRam=observeSystemMemory(core,minimumSystemRam);
  const discs=discControl.activate();core._retro_set_controller_port_device(0,1);
  const av=core._malloc(40);if(!av)fail('AV information allocation failed.');try{core._retro_get_system_av_info(av);readAvInfo(av);}finally{core._free(av);}return discs;
}
function stopGuest(){if(!guestCalls?.healthy)throw Object.assign(new Error('The emulator call state is no longer usable.'),{unrecoverable:true});hardwareVideo?.beforeGuestStop();if(loaded){guestCalls.unloadGame();loaded=false;}if(initialized){guestCalls.deinit();initialized=false;}hardwareVideo?.afterGuestStop();discControl.clear();}
function bootGuest(){clearInput();videoWidth=0;videoHeight=0;hardwareVideo?.reinitialize(playbackPreset);initializeGuest();loadPreparedGame();}

function tick(){
  if(!running||!core)return;
  try{
    if(!guestCalls?.healthy)throw new Error('The emulator call state is no longer usable.');
    const before=performance.now();if(lastInputAt>=0&&before-lastInputAt>250)clearInput();if(previousFrameAt)frameTimes.push(before-previousFrameAt);previousFrameAt=before;
    hardwareVideo?.beforeRun();guestCalls.run();flushSamples();
    coreRunTimes.push(performance.now()-before);framesEmulated++;
    const now=performance.now();flushRumble(now);deadline+=framePeriod;
    if(now-deadline>framePeriod*2){droppedFrames+=Math.floor((now-deadline)/framePeriod);deadline=now;}
    if(now-lastMetrics>=1000){const sorted=frameTimes.filter(Number.isFinite).sort((a,b)=>a-b),runs=coreRunTimes.sort((a,b)=>a-b);const q=(values,percent)=>values.length?values[Math.min(values.length-1,Math.floor((values.length-1)*percent))]:0;const samples=collectMeasurement?Float32Array.from(frameTimes).buffer:null;if(!metricsPending&&metricsSequence<Number.MAX_SAFE_INTEGER){metricsPending=++metricsSequence;emit('metrics',{sequence:metricsPending,framesPresented:frames,framesEmulated,droppedFrames,audioDroppedFrames,nominalRefreshHz:1000/framePeriod,presentationMaxHz,windowDurationMs:now-lastMetrics,frameTimeP50Ms:q(sorted,.5),frameTimeP95Ms:q(sorted,.95),frameTimeP99Ms:q(sorted,.99),coreRunP95Ms:q(runs,.95),frameIntervalsMs:samples},samples?[samples]:[]);}frames=0;framesEmulated=0;droppedFrames=0;audioDroppedFrames=0;frameTimes=[];coreRunTimes=[];lastMetrics=now;}
    timer=setTimeout(tick,Math.max(0,deadline-performance.now()));
  }catch(error){terminal=true;running=false;clearRumble();emit('runtime_error',{code:error.code||'runtime_crashed',message:error.message||'The emulator stopped unexpectedly.'});}
}

function baseSnapshot(kind){
  if(!core)fail('Runtime is not prepared.');
  if(kind==='in_game'){
    const ramSize=core._retro_get_memory_size(0),ram=core._retro_get_memory_data(0),rtcSize=core._retro_get_memory_size(1),rtc=core._retro_get_memory_data(1);
    if(!ramSize&&!rtcSize&&!savedFileSession)fail('This game has no available in-game save memory.');
    if(ramSize+rtcSize+16>MAX_SAVE||(ramSize&&(!ram||ram+ramSize>core.HEAPU8.length))||(rtcSize&&(!rtc||rtc+rtcSize>core.HEAPU8.length)))fail('Save memory exceeds policy.');
    const output=new Uint8Array(16+ramSize+rtcSize),header=new DataView(output.buffer);output.set(new TextEncoder().encode('OPRM0001'));header.setUint32(8,ramSize,true);header.setUint32(12,rtcSize,true);if(ramSize)output.set(core.HEAPU8.subarray(ram,ram+ramSize),16);if(rtcSize)output.set(core.HEAPU8.subarray(rtc,rtc+rtcSize),16+ramSize);return output.buffer;
  }
  if(kind!=='emulator_state')fail('Unknown save type.');
  const disc=discControl?.enabled?discControl.status():null,raw=rawSnapshot();
  if(disc){if(!discControl.same(disc))fail('The disc state changed while creating a snapshot.');return discEnvelope(raw,disc);}return raw;
}
function rawSnapshot(){
  const size=guestCalls.serializeSize();if(!size||size>MAX_SAVE-(discControl?.enabled?24:0))fail('Save state size is unsupported.');
  const pointer=core._malloc(size);if(!pointer)fail('Save state allocation failed.');
  try{if(!guestCalls.serialize(pointer,size))fail('The core could not create this save state.');return core.HEAPU8.slice(pointer,pointer+size).buffer;}finally{if(guestCalls.healthy)core._free(pointer);}
}

function parseBase(kind,payload){
  const bytes=payload instanceof ArrayBuffer?new Uint8Array(payload):payload;
  if(!core||!(bytes instanceof Uint8Array)||bytes.byteLength===0||bytes.byteLength>MAX_SAVE)fail('Save payload is invalid.');
  if(kind==='in_game'){
    if(bytes.byteLength<16||new TextDecoder().decode(bytes.subarray(0,8))!=='OPRM0001')fail('In-game save envelope is unsupported.');
    const header=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength),ramSize=header.getUint32(8,true),rtcSize=header.getUint32(12,true),ram=core._retro_get_memory_data(0),rtc=core._retro_get_memory_data(1);
    if(ramSize!==core._retro_get_memory_size(0)||rtcSize!==core._retro_get_memory_size(1)||16+ramSize+rtcSize!==bytes.byteLength||(ramSize&&(!ram||ram+ramSize>core.HEAPU8.length))||(rtcSize&&(!rtc||rtc+rtcSize>core.HEAPU8.length)))fail('In-game save does not match this game.');
    return{bytes,ramSize,rtcSize};
  }
  if(kind!=='emulator_state')fail('Unknown save type.');
  return discControl?.enabled?discControl.parse(bytes):{raw:bytes,disc:null};
}
function applyBase(kind,value){
  if(kind!=='in_game')return applySnapshot(value);
  // A reboot may expose different memory regions; validate again before writes.
  const{bytes,ramSize,rtcSize}=parseBase(kind,value.bytes),ram=core._retro_get_memory_data(0),rtc=core._retro_get_memory_data(1);
  if(ramSize)core.HEAPU8.set(bytes.subarray(16,16+ramSize),ram);if(rtcSize)core.HEAPU8.set(bytes.subarray(16+ramSize),rtc);return true;
}
function snapshot(kind){
  if(!core||!loaded)fail('The game is not loaded.');
  if(!savedFileSession)return baseSnapshot(kind);
  const resume=running;running=false;clearTimeout(timer);timer=null;
  try{return savedFileSession.capture(kind,baseSnapshot);}
  finally{if(resume&&guestCalls.healthy){running=true;deadline=performance.now();previousFrameAt=0;timer=setTimeout(tick,framePeriod);}}
}
function load(kind,payload){
  if(!core||!loaded||!(payload instanceof ArrayBuffer)||!payload.byteLength||payload.byteLength>MAX_SAVE)fail('Save payload is invalid.');
  if(savedFileSession){savedFileSession.restore(kind,payload,{parseBase,captureBase:baseSnapshot,applyBase,stopGuest,bootGuest});return;}
  const target=parseBase(kind,payload);if(kind==='in_game'){applyBase(kind,target);return;}const before=parseBase(kind,baseSnapshot('emulator_state'));
  let accepted=false;try{accepted=applySnapshot(target);}catch{}
  if(accepted)return;
  let recovered=false;try{recovered=applySnapshot(before);}catch{}
  if(!recovered)throw Object.assign(new Error('The emulator could not recover its previous core and disc state after rejecting a snapshot.'),{unrecoverable:true});
  fail('The emulator rejected this snapshot. Its previous core and disc state were restored.');
}
function applySnapshot(value){
  if(value.disc)discControl.apply(value.disc);
  const pointer=core._malloc(value.raw.byteLength);if(!pointer)fail('Save loading allocation failed.');
  let accepted=false;try{core.HEAPU8.set(value.raw,pointer);accepted=Boolean(guestCalls.unserialize(pointer,value.raw.byteLength));}finally{if(guestCalls.healthy)core._free(pointer);}
  return accepted&&(!value.disc||discControl.same(value.disc));
}

async function stop(){
  running=false;clearInput();clearTimeout(timer);timer=null;
  collectMeasurement=false;
  let cleanupError=false;
  if(core&&!terminal&&guestCalls?.healthy){
    try{if(loaded&&savedFileSession)savedFileSession.flush();}catch(error){cleanupError=true;if(error.unrecoverable)terminal=true;}
    if(!terminal){try{stopGuest();}catch{cleanupError=true;terminal=true;}}
    if(!terminal){
      if(savedFileSession){try{savedFileSession.dispose();}catch{cleanupError=true;}savedFileSession=null;}
      if(!threadPool){try{if(activeGameInfo)core._free(activeGameInfo);if(romPointer)core._free(romPointer);for(const pointer of persistentStrings)core._free(pointer);for(const pointer of callbackPointers)core.removeFunction(pointer);}catch{cleanupError=true;terminal=true;}}
    }
  }else if(core)cleanupError=true;
  if(hardwareVideo){try{hardwareVideo.detach(!terminal);}catch{cleanupError=true;terminal=true;}hardwareVideo=null;}
  if(guestCalls){if(!terminal&&!threadPool){try{guestCalls.dispose();}catch{cleanupError=true;}}guestCalls=null;}
  threadCallbacks?.close();threadCallbacks=null;threadPool?.close();threadPool=null;
  let storagePending=false;if(mediaStage){const stage=mediaStage;mediaStage=null;try{storagePending=(await stage.dispose()).cleanup_pending;}catch{storagePending=true;}}
  if(unmountFirmware){try{unmountFirmware();}catch{cleanupError=true;}unmountFirmware=null;}
  if(discControl){try{discControl.clear();}catch{cleanupError=true;}discControl=null;}
  hapticsEnabled=false;rumblePointer=0;hapticsAvailable=[false,false,false,false];lastHapticsAt=0;pointerEnabled=false;videoWidth=0;videoHeight=0;core=null;initialized=false;loaded=false;frontendCallbacks=null;savedFileSession=null;hardwarePolicy=null;canvas=null;context=null;activeGameInfo=0;romPointer=0;pathPointer=0;callbackPointers=[];persistentStrings=[];stringCache.clear();coreVariables.clear();variableDefinitions.clear();input=Array.from({length:4},()=>new Int16Array(16));
  if(storagePending)emit('runtime_storage_warning',{message:'Temporary game files could not all be removed. A later game opening will retry bounded staging cleanup.'});
  return{cleanup_pending:storagePending,core_cleanup_failed:cleanupError};
}

async function handleMessage(message){
  const{id,method,params={}}=message||{};
  if(method==='audio_ack'){if(Number.isSafeInteger(params.sequence))audioPending.delete(params.sequence);return;}
  if(method==='video_ack'){if(params.sequence===videoPending&&typeof params.presented==='boolean'){videoPending=0;if(params.presented)frames++;else droppedFrames++;}return;}
  if(method==='metrics_ack'){if(params.sequence===metricsPending)metricsPending=0;return;}
  if(method==='av_ack'){if(params.sequence===avPending){avPending=0;const next=avQueued;avQueued=null;if(next)publishAv(next);}return;}
  if(method==='haptics_ack'){
    if(params&&Number.isSafeInteger(params.sequence)&&params.sequence===hapticsPending){hapticsPending=0;const next=hapticsQueued;hapticsQueued=null;if(next)publishRumble(next.values,next.at);}return;
  }
  if(typeof id!=='string'||typeof method!=='string')return;
  if(terminal&&method!=='stop'){self.postMessage({id,status:'error',error:{code:'runtime_crashed',message:'The emulator stopped. Reopen the game before continuing.'}});return;}
  try{
    let result={};let transfer=[];
    switch(method){
      case'media_begin':if(core||mediaStage)fail('Media staging is already in use.');mediaStage=await MediaStage.create(params);result={stage_id:mediaStage.id,chunk_bytes:48*1024,media_protocol:1};break;
      case'media_asset_start':if(!mediaStage||core)fail('Media staging is unavailable.');result=await mediaStage.startAsset(params);break;
      case'media_chunk':if(!mediaStage||core)fail('Media staging is unavailable.');result=mediaStage.chunk(params);break;
      case'media_asset_finish':if(!mediaStage||core)fail('Media staging is unavailable.');result=await mediaStage.finishAsset(params);break;
      case'media_seal':if(!mediaStage||core)fail('Media staging is unavailable.');result=mediaStage.seal(params);break;
      case'prepare':result=await prepare(params);break;
      case'measurement':if(typeof params.enabled!=='boolean')fail('Measurement selection is invalid.');collectMeasurement=params.enabled;break;
      case'start':case'resume':if(!core)fail('Runtime is not prepared.');if(!running){running=true;deadline=performance.now();previousFrameAt=0;nextPresentationAt=0;lastMetrics=deadline;frames=0;framesEmulated=0;droppedFrames=0;audioDroppedFrames=0;frameTimes=[];coreRunTimes=[];tick();}break;
      case'pause':running=false;sampleCount=0;clearInput();clearTimeout(timer);timer=null;break;
      case'stop':result=await stop();break;
      case'input':{
        if(!Array.isArray(params.hapticsAvailable)||params.hapticsAvailable.length!==4||params.hapticsAvailable.some(v=>typeof v!=='boolean')||!Array.isArray(params.ports)||params.ports.length!==4||params.ports.some(values=>!Array.isArray(values)||values.length!==16||values.some(value=>value!==0&&value!==1))||!Array.isArray(params.analogPorts)||params.analogPorts.length!==4||params.analogPorts.some(values=>!Array.isArray(values)||values.length!==4||values.some(value=>!Number.isInteger(value)||value< -32767||value>32767))||!Number.isFinite(params.observed_at_ms))fail('Input timestamp, capability, axis or button shape is invalid.');
        const point=params.pointer;if(!point||typeof point!=='object'||Array.isArray(point)||Object.keys(point).length!==4||Object.keys(point).some(key=>!['x','y','pressed','inside'].includes(key))||!Number.isInteger(point.x)||point.x< -32767||point.x>32767||!Number.isInteger(point.y)||point.y< -32767||point.y>32767||typeof point.pressed!=='boolean'||typeof point.inside!=='boolean'||point.pressed&&!point.inside)fail('Pointer input shape is invalid.');
        const geometry=params.pointerGeometry;if(!geometry||typeof geometry!=='object'||Array.isArray(geometry)||Object.keys(geometry).length!==2||Object.keys(geometry).some(key=>!['width','height'].includes(key))||!Number.isInteger(geometry.width)||!Number.isInteger(geometry.height)||geometry.width<0||geometry.height<0||geometry.width>4096||geometry.height>4096||Boolean(geometry.width)!==Boolean(geometry.height))fail('Pointer geometry shape is invalid.');
        result={videoGeometry:{width:videoWidth,height:videoHeight}};const now=performance.now(),age=performance.timeOrigin+now-params.observed_at_ms;if(!running||age< -50||age>250){clearInput();break;}
        lastInputAt=now-Math.max(0,age);hapticsAvailable=params.hapticsAvailable;pointerInput=pointerEnabled&&videoWidth>0&&geometry.width===videoWidth&&geometry.height===videoHeight?{...point}:{x:0,y:0,pressed:false,inside:false};for(let port=0;port<4;port++){input[port]=Int16Array.from(params.ports[port]);analogInput[port]=Int16Array.from(params.analogPorts[port]);if(!hapticsAvailable[port])rumble[port]={strong:0,weak:0};}break;
      }
      case'save':hardwareVideo?.beforeRun();result={payload:snapshot(params.kind)};hardwareVideo?.beforeRun();transfer=[result.payload];break;
      case'load':if(running)fail('Pause the game before loading saved progress.');hardwareVideo?.beforeRun();load(params.kind,params.payload);hardwareVideo?.beforeRun();break;
      case'disc_status':if(!core||!loaded)fail('The game is not loaded.');result=discControl.status();break;
      case'disc_select':if(!core||!loaded||running)fail('Pause the loaded game before changing discs.');result=discControl.select(params.index);break;
      case'reset':if(!core)fail('Runtime is not prepared.');guestCalls.reset();break;
      default:fail('Unknown runtime command.');
    }
    threadCallbacks?.check();
    self.postMessage({id,status:'ok',result},transfer);
  }catch(error){if(error.unrecoverable){terminal=true;running=false;clearTimeout(timer);clearInput();emit('runtime_error',{code:'runtime_crashed',message:error.message});}self.postMessage({id,status:'error',error:{code:error.code||'runtime_crashed',message:error.message||'Runtime operation failed.'}});}
}
let commands=Promise.resolve(),queuedCommands=0;
self.onmessage=event=>{
  const message=event.data;
  if(message?.method==='pthread_message_v1'){threadPool?.receive(message.params);return;}
  if(['haptics_ack','audio_ack','video_ack','metrics_ack','av_ack'].includes(message?.method)){void handleMessage(message);return;}
  if(typeof message?.id!=='string'||typeof message?.method!=='string')return;
  if(queuedCommands>=32){self.postMessage({id:message.id,status:'error',error:{code:'resource_limit',message:'The isolated runtime command queue is full.'}});return;}
  queuedCommands++;commands=commands.then(()=>handleMessage(message)).catch(()=>{}).finally(()=>{queuedCommands--;});
};
