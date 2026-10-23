import{StreamError,requireValue,UUID}from'./crypto.js';
import{PROFILES}from'./qos.js';

const cancelled=()=>new StreamError('cancelled','The capture selection was cancelled.');
const stopTracks=stream=>{for(const track of stream?.getTracks?.()??[])try{track.stop();}catch{}};
export function nativeWindowCaptureAvailable(){
  return globalThis.location?.protocol==='chrome-extension:'&&globalThis.window===globalThis.window?.top&&
    location.hostname===globalThis.chrome?.runtime?.id&&typeof chrome.desktopCapture?.chooseDesktopMedia==='function'&&
    typeof chrome.desktopCapture?.cancelChooseDesktopMedia==='function'&&typeof navigator.mediaDevices?.getUserMedia==='function';
}
export function displayCaptureAvailable(){return typeof globalThis.navigator?.mediaDevices?.getDisplayMedia==='function';}
export function captureAvailable(){return nativeWindowCaptureAvailable()||displayCaptureAvailable();}

function measurement(value,ceiling){return typeof value==='number'&&Number.isFinite(value)&&value>0&&value<=ceiling?value:null;}
export class CaptureSource extends EventTarget{
  #stream;#backend;#requestedAudio;#profile;#closed=false;#events=new AbortController();
  #tracks=[];#video;
  #audioScope='user_selected_unspecified';
  #owned=null;#onRetire=null;#expiry=0;
  constructor(stream,{backend='display_picker',audio=false,profile,audioScope='user_selected_unspecified',owned=null,onRetire=null}={}){
    super();
    try{
      requireValue(typeof MediaStream==='function'&&stream instanceof MediaStream&&PROFILES[profile]&&
        ['display_picker','chromium_window_picker'].includes(backend)&&
        ['user_selected_unspecified','browser_reported_application'].includes(audioScope),
        'invalid_request','A live capture stream and finite quality profile are required.');
      const tracks=stream.getTracks(),video=stream.getVideoTracks(),sound=stream.getAudioTracks();
      requireValue(tracks.length>=1&&tracks.length<=2&&video.length===1&&sound.length<=1&&
        tracks.every(track=>['video','audio'].includes(track.kind)&&track.readyState==='live')&&
        (audio||sound.length===0),'unsupported_capability','The capture returned an unexpected track set.');
      this.#stream=stream;this.#tracks=[...tracks];this.#video=video[0];this.#backend=backend;this.#requestedAudio=audio;this.#profile=profile;this.#audioScope=audioScope;
      if(owned){requireValue(owned.protocol===1&&[owned.capture_id,owned.game_id,owned.session_id].every(value=>typeof value==='string'&&UUID.test(value))&&Number.isSafeInteger(owned.expires_at_ms)&&owned.expires_at_ms>Date.now()&&owned.expires_at_ms-Date.now()<=30*60*1000&&backend==='chromium_window_picker','unauthorized','The owned capture identity is invalid.');this.#owned=Object.freeze({...owned});this.#onRetire=onRetire;this.#expiry=setTimeout(()=>{this.stop();this.dispatchEvent(new Event('ended'));},owned.expires_at_ms-Date.now());}
      const options={signal:this.#events.signal};
      video[0].addEventListener('ended',()=>{this.stop();this.dispatchEvent(new Event('ended'));},options);
      for(const track of tracks){
        for(const name of ['mute','unmute','ended'])track.addEventListener(name,()=>this.#changed(),options);
      }
    }catch(error){stopTracks(stream);throw error;}
  }
  get stream(){return this.#stream;}
  get closed(){return this.#closed;}
  async configure(){
    requireValue(!this.#closed,'cancelled','The capture source was closed.');
    const video=this.#video,profile=PROFILES[this.#profile];
    // The browser's picker controls source selection. Downscale/frame limits
    // apply only to the already chosen track, never to the set of offered sources.
    await video.applyConstraints({width:{max:profile.width},height:{max:profile.height},frameRate:{max:profile.fps}});
    requireValue(!this.#closed&&video.readyState==='live','cancelled','The capture source ended before it was ready.');
    const settings=video.getSettings();
    requireValue((settings.width===undefined||measurement(settings.width,profile.width)!==null)&&
      (settings.height===undefined||measurement(settings.height,profile.height)!==null)&&
      (settings.frameRate===undefined||measurement(settings.frameRate,profile.fps+.01)!==null),
      'unsupported_capability','The selected source cannot satisfy this capture profile.');
    video.contentHint='motion';this.#changed();return this;
  }
  snapshot(){
    const video=this.#video;let settings={};try{settings=video.getSettings();}catch{}
    const audio=this.#tracks.some(track=>track.kind==='audio'&&track.readyState==='live');
    return{protocol:1,profile:this.#profile,backend:this.#backend,active:!this.#closed&&video.readyState==='live',muted:video.muted,
      width:measurement(settings.width,8192),height:measurement(settings.height,8192),fps:measurement(settings.frameRate,240),
      display_surface:['monitor','window','browser'].includes(settings.displaySurface)?settings.displaySurface:null,
      audio_requested:this.#requestedAudio,audio_present:audio,audio_scope:audio?this.#audioScope:'none',owned_capture:this.#owned?{...this.#owned}:null};
  }
  #changed(){this.dispatchEvent(new CustomEvent('change',{detail:this.snapshot()}));}
  stop(){if(this.#closed)return;this.#closed=true;clearTimeout(this.#expiry);this.#events.abort();for(const track of this.#tracks)try{track.stop();}catch{}const retire=this.#onRetire;this.#onRetire=null;try{retire?.();}catch{}this.#changed();}
}

export class CaptureRequest{
  #abort=new AbortController();#picker=null;#timer=0;#external=null;#externalListener=null;
  #audioScope='user_selected_unspecified';
  #ownedTarget=null;#ownedVerified=null;
  result;
  constructor(profile,{source='display',audio=false,signal,owned=null}={}){
    requireValue(PROFILES[profile]&&['display','window'].includes(source)&&typeof audio==='boolean',
      'invalid_request','The capture profile or source selection is invalid.');
    requireValue(!signal?.aborted,'cancelled','The remote request was closed.');
    if(owned){requireValue(source==='window'&&[owned.capture_id,owned.game_id,owned.session_id].every(value=>typeof value==='string'&&UUID.test(value)),'invalid_request','Owned capture requires a prepared game window.');this.#ownedTarget=Object.freeze({...owned});}
    requireValue(!navigator.userActivation||navigator.userActivation.isActive,'permission_denied','Select a capture source from a local user action.');
    if(signal){this.#external=signal;this.#externalListener=()=>this.cancel();signal.addEventListener('abort',this.#externalListener,{once:true});}
    this.#timer=setTimeout(()=>this.cancel(),45000);
    let aborted;
    const cancellation=new Promise((_,reject)=>{aborted=()=>reject(cancelled());this.#abort.signal.addEventListener('abort',aborted,{once:true});});
    let requested;
    try{
      // Both picker calls occur synchronously inside this constructor, before
      // awaiting anything that could consume the local transient user activation.
      requested=this.#ownedTarget?this.#ownedWindow(audio):source==='window'?this.#window(audio):this.#display(audio);
    }catch(error){requested=Promise.reject(error);}
    const raw=Promise.resolve(requested);
    raw.then(stream=>{if(this.#abort.signal.aborted)stopTracks(stream);},()=>{});
    this.result=Promise.race([raw,cancellation]).then(async stream=>{
      if(this.#abort.signal.aborted){stopTracks(stream);throw cancelled();}
      const owned=new CaptureSource(stream,{backend:source==='window'?'chromium_window_picker':'display_picker',audio,profile,audioScope:this.#audioScope,owned:this.#ownedVerified,onRetire:()=>this.#releaseOwned()});
      const retire=()=>owned.stop();this.#abort.signal.addEventListener('abort',retire,{once:true});
      try{
        await Promise.race([owned.configure(),cancellation]);
        requireValue(!this.#abort.signal.aborted&&!owned.closed,'cancelled','The capture request was closed.');
        return owned;
      }catch(error){owned.stop();throw error;}
      finally{this.#abort.signal.removeEventListener('abort',retire);}
    }).catch(error=>{
      this.#releaseOwned();
      if(this.#abort.signal.aborted||error?.name==='AbortError'||error?.name==='NotAllowedError')throw cancelled();
      if(error instanceof StreamError)throw error;
      throw new StreamError('unsupported_capability','The selected capture source or audio is unavailable. Choose another source or disable audio.');
    }).finally(()=>{
      clearTimeout(this.#timer);this.#timer=0;this.#abort.signal.removeEventListener('abort',aborted);
      if(this.#external&&this.#externalListener)this.#external.removeEventListener('abort',this.#externalListener);
      this.#external=null;this.#externalListener=null;
    });
  }
  #display(audio){
    requireValue(displayCaptureAvailable(),'unsupported_capability','This device cannot show a display capture picker.');
    return navigator.mediaDevices.getDisplayMedia({video:true,audio,
      selfBrowserSurface:'exclude',systemAudio:audio?'include':'exclude',windowAudio:audio?'window':'exclude',surfaceSwitching:'exclude'});
  }
  async #ownedWindow(audio){
    requireValue(nativeWindowCaptureAvailable()&&typeof chrome.runtime.sendMessage==='function','unsupported_capability','Owned game capture requires the native Chromium component.');
    const target=this.#ownedTarget,value=await chrome.runtime.sendMessage({type:'omniplay_owned_capture_v1',action:'choose',capture_id:target.capture_id,audio});
    requireValue(!this.#abort.signal.aborted,'cancelled','The capture request was closed.');
    requireValue(value&&!value.error&&typeof value.stream_id==='string'&&value.stream_id.length>0&&value.stream_id.length<=4096&&typeof value.audio_present==='boolean'&&value.owned_capture?.protocol===1&&value.owned_capture.capture_id===target.capture_id&&value.owned_capture.game_id===target.game_id&&value.owned_capture.session_id===target.session_id&&value.owned_capture.expires_at_ms===target.expires_at_ms,'unauthorized','The selected window could not be bound to the prepared game.');
    this.#ownedVerified=value.owned_capture;if(value.audio_present)this.#audioScope='browser_reported_application';
    const mandatory={chromeMediaSource:'desktop',chromeMediaSourceId:value.stream_id};
    const stream=await navigator.mediaDevices.getUserMedia({video:{mandatory},audio:audio&&value.audio_present?{mandatory:{...mandatory}}:false});
    if(this.#abort.signal.aborted){stopTracks(stream);throw cancelled();}return stream;
  }
  #releaseOwned(){if(this.#ownedTarget){const target=this.#ownedTarget;this.#ownedTarget=null;void chrome.runtime.sendMessage({type:'omniplay_owned_capture_v1',action:'cancel',capture_id:target.capture_id}).catch(()=>{});}}
  #window(audio){
    requireValue(nativeWindowCaptureAvailable(),'unsupported_capability','This app cannot show the Chromium window picker.');
    return new Promise((resolve,reject)=>{
      let completed=false;
      const callback=(id,options)=>{
        if(completed)return;
        completed=true;this.#picker=null;
        const error=chrome.runtime.lastError;
        if(error){reject(new StreamError('unsupported_capability',audio?'Application audio is unavailable in this app or source. Disable audio or choose another source.':'The Chromium window picker could not open.'));return;}
        if(this.#abort.signal.aborted||!id){reject(cancelled());return;}
        if(typeof id!=='string'||id.length>4096||typeof options?.canRequestAudioTrack!=='boolean'){
          reject(new StreamError('invalid_state','The capture picker returned an invalid source grant.'));return;
        }
        if(audio&&options.canRequestAudioTrack){
          if(options.omniplayCaptureProtocol!==1||options.audioScope!=='application'){
            reject(new StreamError('unsupported_capability','The selected window did not authorize application-only audio. Disable audio or choose another source.'));return;
          }
          this.#audioScope='browser_reported_application';
        }
        // The one-use source token is consumed locally, never persisted, logged,
        // sent over signaling or supplied to a different extension/tab origin.
        const mandatory={chromeMediaSource:'desktop',chromeMediaSourceId:id};
        let capture;
        try{capture=navigator.mediaDevices.getUserMedia({video:{mandatory},
          audio:audio&&options.canRequestAudioTrack?{mandatory:{...mandatory}}:false});}
        catch(error){reject(error);return;}
        Promise.resolve(capture).then(stream=>{
          if(this.#abort.signal.aborted){stopTracks(stream);reject(cancelled());}else resolve(stream);
        },reject);
      };
      // Explicitly supply the absent targetTab so the following dictionary is
      // the pinned API's options argument, and the stream stays in this origin.
      const pickerOptions={systemAudio:'exclude',windowAudio:audio?'window':'exclude',selfBrowserSurface:'exclude'};
      // An unsupported app build must reject this option, never silently widen
      // a request for application audio to the complete system audio mix.
      if(audio)pickerOptions.omniplayStrictWindowAudio=true;
      const id=chrome.desktopCapture.chooseDesktopMedia(audio?['window','audio']:['window'],undefined,pickerOptions,callback);
      if(!completed){
        if(!Number.isSafeInteger(id)||id<0||id>2147483647){reject(new StreamError('invalid_state','The capture picker did not create a valid request.'));return;}
        this.#picker=id;
      }
    });
  }
  cancel(){
    if(this.#abort.signal.aborted)return;this.#abort.abort();
    this.#releaseOwned();
    if(this.#picker!==null){const id=this.#picker;this.#picker=null;try{chrome.desktopCapture.cancelChooseDesktopMedia(id);}catch{}}
  }
}
