import{MAX_ROM,requireValue,checkAbort,WebPlatformError,fail}from'./errors.js';
import{discFolderSelection}from'./disc-source.js';

const TTL=300000;
const hintFor=name=>({chd:'chd',iso:'iso',pbp:'pbp',cso:'cso',z64:'n64_z64',v64:'n64_v64',n64:'n64_n64',cue:'cue',m3u:'m3u',m3u8:'m3u'})[name.split('.').at(-1).toLowerCase()]??null;
function picker(signal,directory=false){
  checkAbort(signal);return new Promise((resolve,reject)=>{
    const input=document.createElement('input');input.type='file';input.hidden=true;let complete=false;
    if(directory){if(!('webkitdirectory'in input)){reject(new WebPlatformError('unsupported_capability','This browser does not expose folder selection. Use a supported browser or the native app.'));return;}input.webkitdirectory=true;input.multiple=true;}
    const finish=(file,error)=>{if(complete)return;complete=true;input.remove();signal?.removeEventListener('abort',cancel);error?reject(error):resolve(file);};
    const cancel=()=>finish(null,new WebPlatformError('cancelled','No file was selected.'));signal?.addEventListener('abort',cancel,{once:true});
    input.addEventListener('change',()=>{try{if(directory)finish(discFolderSelection(input.files,signal));else if(input.files?.length===1)finish(input.files[0]);else cancel();}catch(error){finish(null,error);}},{once:true});input.addEventListener('cancel',cancel,{once:true});document.body.append(input);try{input.click();}catch(error){finish(null,error);}
  });
}
export class WebFiles {
  #grants=new Map();#links=new Map();#expiryTimer=0;
  supportsDisc(){return'webkitdirectory'in document.createElement('input');}
  #scheduleExpiry(){clearTimeout(this.#expiryTimer);this.#expiryTimer=0;if(this.#grants.size){const next=Math.min(...Array.from(this.#grants.values(),grant=>grant.expires));this.#expiryTimer=setTimeout(()=>this.#expire(),Math.max(1,next-performance.now()+1));}}
  #expire(){const now=performance.now();for(const[id,grant]of this.#grants)if(grant.expires<=now)this.#grants.delete(id);this.#scheduleExpiry();}
  #add(grant){this.#expire();requireValue(this.#grants.size<32&&(grant.kind!=='disc'||Array.from(this.#grants.values()).filter(item=>item.kind==='disc').length<4),'resource_limit','Too many pending file or folder selections.');const id=crypto.randomUUID();this.#grants.set(id,{...grant,expires:performance.now()+TTL});this.#scheduleExpiry();return id;}
  async selectImport(signal){
    const file=await picker(signal);requireValue(file.size>0&&file.size<=MAX_ROM+64*1024+12,'resource_limit','The selected file exceeds the import bound.');checkAbort(signal);
    const grant_id=this.#add({kind:'import',file});return{grant_id,suggested_title:file.name.replace(/\.[^.]+$/,'').slice(0,120)||'Game',format_hint:hintFor(file.name)};
  }
  async selectDisc(signal){const scope=await picker(signal,true);checkAbort(signal);const grant_id=this.#add({kind:'disc',scope});return{grant_id,suggested_title:scope.suggested_title,entries:[...scope.entries]};}
  takeDisc(id,entry){this.#expire();const grant=this.#grants.get(id);requireValue(grant?.kind==='disc'&&grant.scope.entries.includes(entry),'permission_expired','Choose the disc folder again and select one of its offered entry points.');this.#grants.delete(id);this.#scheduleExpiry();return grant.scope;}
  async selectExport(name,signal){
    requireValue(typeof name==='string'&&/^[A-Za-z0-9_.-]{1,128}$/.test(name)&&name!=='.'&&name!=='..','invalid_request','Export filename is invalid.');checkAbort(signal);let handle=null;
    if(typeof globalThis.showSaveFilePicker==='function'){
      try{handle=await globalThis.showSaveFilePicker({suggestedName:name});}catch(error){if(error.name==='AbortError')fail('cancelled','No export destination was selected.');fail('permission_denied','The browser could not authorize that export destination.');}
    }
    checkAbort(signal);return{grant_id:this.#add({kind:'export',name,handle})};
  }
  take(id,kind){this.#expire();const grant=this.#grants.get(id);requireValue(grant&&grant.kind===kind,'permission_expired','File selection has expired or was already used.');this.#grants.delete(id);this.#scheduleExpiry();return grant;}
  revoke(id){this.#grants.delete(id);this.#scheduleExpiry();return{revoked:true};}
  async write(id,blob,signal,{guard=null}={}){
    requireValue(blob instanceof Blob&&(guard===null||typeof guard==='function'),'invalid_request','Export data is invalid.');const grant=this.take(id,'export');return this.deliver(grant,blob,signal,{guard});
  }
  async deliver(grant,blob,signal,{guard=null}={}){
    const check=async()=>{checkAbort(signal);if(guard)await guard();checkAbort(signal);};await check();
    if(grant.handle){
      let writer;try{writer=await grant.handle.createWritable({keepExistingData:false});await check();await writer.write(blob);await check();await writer.close();return{exported:true,delivery:'file_handle'};}
      catch(error){await writer?.abort().catch(()=>{});if(signal?.aborted)fail('cancelled','Export was cancelled before completion.');if(error instanceof WebPlatformError)throw error;fail('storage_failure','The selected export could not be completed.');}
    }
    // Some browsers expose downloads rather than a writable file picker. Keep
    // a visible retry link; initiating a download is not an fsync acknowledgement.
    requireValue(this.#links.size<4,'resource_limit','Use or dismiss the existing export downloads before creating another.');
    const url=URL.createObjectURL(blob),link=document.createElement('a');link.href=url;link.download=grant.name;link.className='button button-secondary';link.textContent=`↓ ${grant.name}`;
    const container=document.querySelector('dialog[open] [data-export-downloads]')??document.querySelector('[data-web-downloads]')??document.body,group=document.createElement('span'),dismiss=document.createElement('button');dismiss.type='button';dismiss.textContent='×';dismiss.setAttribute('aria-label','Dismiss download');
    const clear=()=>{URL.revokeObjectURL(url);group.remove();this.#links.delete(url);};dismiss.addEventListener('click',clear,{once:true});group.append(link,dismiss);container.append(group);link.click();this.#links.set(url,group);
    setTimeout(clear,300000);return{exported:true,delivery:'browser_download_started'};
  }
  close(){clearTimeout(this.#expiryTimer);this.#expiryTimer=0;this.#grants.clear();for(const[url,link]of this.#links){URL.revokeObjectURL(url);link.remove();}this.#links.clear();}
}
