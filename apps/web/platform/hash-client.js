import{WebPlatformError,checkAbort,HASH,integer,requireValue}from'./errors.js';
function hashOperation(blob,{signal,onProgress=()=>{}}={},snapshot=false){
  checkAbort(signal);
  requireValue(blob instanceof Blob&&integer(blob.size,0,4*1024**3),'resource_limit','The content snapshot exceeds its size limit.');
  return new Promise((resolve,reject)=>{
    const worker=new Worker(new URL('./hash-worker.js',import.meta.url),{type:'module',name:'omniplay-file-hash'}),id=crypto.randomUUID();let finished=false,lastProgress=0;
    const finish=(error,hash)=>{if(finished)return;finished=true;clearTimeout(timer);signal?.removeEventListener('abort',cancel);worker.terminate();error?reject(error):resolve(hash);};
    const cancel=()=>finish(new WebPlatformError('cancelled','File import was cancelled.'));
    const timer=setTimeout(()=>finish(new WebPlatformError('deadline_exceeded','File hashing exceeded its time limit.')),30*60*1000);
    signal?.addEventListener('abort',cancel,{once:true});
    worker.onmessage=event=>{const message=event.data;if(message?.id!==id)return;try{
      if(message.event==='progress'){requireValue(integer(message.bytes,lastProgress,blob.size),'corrupt_data','File hashing progress is invalid.');lastProgress=message.bytes;onProgress(message.bytes);}
      else if(message.event==='complete'){requireValue(typeof message.sha256==='string'&&message.sha256.length===64&&HASH.test(message.sha256)&&(!snapshot||message.blob instanceof Blob&&message.blob.size===blob.size),'corrupt_data','The content snapshot response is invalid.');finish(null,snapshot?{sha256:message.sha256,blob:message.blob}:message.sha256);}
      else if(message.event==='error')finish(new WebPlatformError(message.code==='cancelled'?'cancelled':'corrupt_data','The selected file could not be read completely.'));
    }catch(error){finish(error);}};
    worker.onerror=()=>finish(new WebPlatformError('corrupt_data','The file-hashing worker stopped.'));worker.onmessageerror=()=>finish(new WebPlatformError('corrupt_data','File-hashing response was invalid.'));
    try{worker.postMessage({id,method:'hash',blob,snapshot});}catch(error){finish(error);}
  });
}
export const hashInWorker=(blob,options)=>hashOperation(blob,options,false);
export const snapshotInWorker=(blob,options)=>hashOperation(blob,options,true);
