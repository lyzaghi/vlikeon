import{hashBlob,Sha256}from'./sha256.js';
let active=null;
async function snapshot(blob,signal,onProgress){
  const parts=[],hash=new Sha256();let copied=0;
  for(let at=0;at<blob.size;at+=256*1024){
    if(signal.aborted)throw new DOMException('Cancelled','AbortError');const count=Math.min(256*1024,blob.size-at),bytes=new Uint8Array(await blob.slice(at,at+count).arrayBuffer());
    if(signal.aborted)throw new DOMException('Cancelled','AbortError');if(bytes.byteLength!==count)throw new Error('snapshot_length');
    // Each payload part copies actual read bytes into a new browser-owned Blob.
    // No provider File or original pathname remains in the returned snapshot.
    hash.update(bytes);parts.push(new Blob([bytes],{type:'application/octet-stream'}));copied+=count;onProgress(copied);
  }
  return{sha256:hash.hex(),blob:new Blob(parts,{type:'application/octet-stream'})};
}
self.onmessage=event=>{
  const message=event.data;
  if(message?.method==='cancel'){if(active?.id===message.id)active.controller.abort();return;}
  if(message?.method!=='hash'||active||typeof message.id!=='string'||!(message.blob instanceof Blob)||message.blob.size>4*1024**3||message.snapshot!==undefined&&typeof message.snapshot!=='boolean')return;
  const operation={id:message.id,controller:new AbortController()};active=operation;let last=0;
  const onProgress=bytes=>{const now=performance.now();if(now-last>=100||bytes===message.blob.size){last=now;self.postMessage({id:operation.id,event:'progress',bytes});}};
  const work=message.snapshot?snapshot(message.blob,operation.controller.signal,onProgress):hashBlob(message.blob,{signal:operation.controller.signal,onProgress}).then(sha256=>({sha256}));
  void work.then(result=>self.postMessage({id:operation.id,event:'complete',...result})).catch(error=>{self.postMessage({id:operation.id,event:'error',code:error?.name==='AbortError'?'cancelled':'corrupt_data'});}).finally(()=>{active=null;});
};
