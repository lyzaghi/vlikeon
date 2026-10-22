export class PlatformError extends Error {
  constructor(error = {}) { super(error.message || 'Operation failed'); this.name = 'PlatformError'; this.code = error.code || 'internal'; this.action = error.action; this.recoverable = Boolean(error.recoverable); }
}

const webContext=['https:','http:'].includes(location.protocol)&&window===window.top&&document.querySelector('meta[name="omniplay-platform"]')?.content==='web';
const webEvents=new EventTarget();let webHost=null;
function connectWeb(){
  if(!webHost)webHost=import('./web/platform.js').then(module=>module.openWebPlatform(()=>{webHost=null;webEvents.dispatchEvent(new Event('closed'));})).catch(error=>{webHost=null;throw error;});return webHost;
}

// Android uses Chromium WebContents MessagePort with an exact native target
// origin. No Java object or native method is injected into a browsing page.
const androidContext=location.protocol==='http:'&&location.hostname==='127.0.0.1'&&/^\/h\/[a-f0-9]{48}\/index\.html$/.test(location.pathname);
const androidEvents=new EventTarget();let androidPort=null,androidReadyResolve;
const androidReady=new Promise(resolve=>{androidReadyResolve=resolve;});
if(androidContext)globalThis.addEventListener('message',event=>{
  if(!event.isTrusted||event.origin!==location.origin||event.data!=='omniplay.android.channel.v1'||event.ports.length!==1)return;
  androidPort?.close();androidPort=event.ports[0];
  androidPort.onmessage=async event=>{
    try{
      if(typeof event.data!=='string'||event.data.length>1048576)return;
      const response=JSON.parse(event.data);
      if(response.event==='native_suspend'&&typeof response.suspension_id==='string'){
        try{await globalThis.omniplayLifecycle?.suspend();}finally{androidPort?.postMessage(JSON.stringify({event:'lifecycle_flushed',suspension_id:response.suspension_id}));}
      }else androidEvents.dispatchEvent(new CustomEvent('response',{detail:response}));
    }catch{androidEvents.dispatchEvent(new Event('closed'));}
  };
  androidPort.onmessageerror=()=>androidEvents.dispatchEvent(new Event('closed'));
  androidPort.start();androidReadyResolve();
});
if(androidContext)Object.defineProperty(globalThis,'__omniplayAndroidBridgeReady',{value:true,writable:false,configurable:false});

export class PlatformBridge extends EventTarget {
  #port = null;
  #pending = new Map();
  #mobile = false;
  #android = false;
  #web = false;

  connect() {
    if(webContext){if(!this.#web){this.#web=true;webEvents.addEventListener('closed',()=>this.dispatchEvent(new Event('host_disconnected')));}return;}
    if(globalThis.webkit?.messageHandlers?.omniplay?.postMessage){this.#mobile=true;return;}
    if(androidContext){
      if(this.#android)return;this.#android=true;
      androidEvents.addEventListener('response',event=>{const message=event.detail;if(message.event){this.dispatchEvent(new CustomEvent(message.event,{detail:message}));return;}const item=this.#pending.get(message.id);if(!item)return;this.#pending.delete(message.id);clearTimeout(item.timer);message.status==='ok'?item.resolve(message.result):item.reject(new PlatformError(message.error));});
      androidEvents.addEventListener('closed',()=>{for(const item of this.#pending.values()){clearTimeout(item.timer);item.reject(new PlatformError({code:'runtime_unavailable',message:'The native Game Hub channel closed.'}));}this.#pending.clear();this.dispatchEvent(new Event('host_disconnected'));});
      return;
    }
    if (this.#port) return;
    if (!globalThis.chrome?.runtime?.connect) throw new PlatformError({ code: 'runtime_unavailable', message: 'Open Game Hub in the OmniPlay browser.' });
    this.#port = chrome.runtime.connect({ name: 'omniplay-ui' });
    this.#port.onMessage.addListener(message => {
      if (message.event) { this.dispatchEvent(new CustomEvent(message.event, { detail: message })); return; }
      const item = this.#pending.get(message.id);
      if (!item) return;
      this.#pending.delete(message.id);
      clearTimeout(item.timer);
      message.status === 'ok' ? item.resolve(message.result) : item.reject(new PlatformError(message.error));
    });
    this.#port.onDisconnect.addListener(() => {
      void chrome.runtime.lastError;
      this.#port = null;
      for (const item of this.#pending.values()) { clearTimeout(item.timer); item.reject(new PlatformError({ code: 'runtime_unavailable', message: 'The application connection closed.' })); }
      this.#pending.clear();
      this.dispatchEvent(new Event('host_disconnected'));
    });
  }

  request(method, params, timeout = 30000) {
    this.connect();
    if(this.#web)return connectWeb().then(host=>host.request(method,params,timeout)).catch(error=>{throw error instanceof PlatformError?error:new PlatformError(error);});
    if (this.#pending.size >= 16) return Promise.reject(new PlatformError({ code: 'busy', message: 'Wait for the current operation to finish.' }));
    const id = crypto.randomUUID();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.#pending.delete(id); reject(new PlatformError({ code: 'deadline_exceeded', message: 'Operation timed out.' })); }, timeout + 1000);
      this.#pending.set(id, { resolve, reject, timer });
      const message={protocol_major:1,protocol_minor:0,id,deadline_ms:Date.now()+timeout,command:params===undefined?{method}:{method,params}};
      try {
        if(this.#android){
          androidReady.then(()=>{if(this.#pending.has(id))androidPort.postMessage(JSON.stringify(message));}).catch(error=>{const pending=this.#pending.get(id);if(pending){clearTimeout(pending.timer);this.#pending.delete(id);pending.reject(error);}});
        }else if(this.#mobile){
          Promise.resolve(webkit.messageHandlers.omniplay.postMessage(message)).then(response=>{const pending=this.#pending.get(id);if(!pending)return;clearTimeout(pending.timer);this.#pending.delete(id);if(response?.id!==id){pending.reject(new PlatformError({code:'invalid_request',message:'Native response correlation failed.'}));return;}response.status==='ok'?pending.resolve(response.result):pending.reject(new PlatformError(response.error));}).catch(error=>{const pending=this.#pending.get(id);if(pending){clearTimeout(pending.timer);this.#pending.delete(id);pending.reject(error);}});
        }else this.#port.postMessage(message);
      }
      catch (error) { clearTimeout(timer); this.#pending.delete(id); reject(error); }
    });
  }
}
