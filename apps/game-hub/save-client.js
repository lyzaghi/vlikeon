export async function sha256(buffer){return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',buffer)),byte=>byte.toString(16).padStart(2,'0')).join('');}
function encode(bytes){let binary='';for(let offset=0;offset<bytes.length;offset+=8192)binary+=String.fromCharCode(...bytes.subarray(offset,offset+8192));return btoa(binary);}
function decode(encoded){const binary=atob(encoded),bytes=new Uint8Array(binary.length);for(let index=0;index<binary.length;index++)bytes[index]=binary.charCodeAt(index);return bytes;}

export async function download(bridge,info,{maxBytes=256*1024*1024,onProgress=()=>{},signal}={}){
  try{
    const cancelled=()=>{if(signal?.aborted)throw Object.assign(new Error('Content transfer was cancelled.'),{code:'cancelled'});};cancelled();
    if(!info||!Number.isSafeInteger(info.byte_length)||info.byte_length<=0||info.byte_length>maxBytes||typeof info.sha256!=='string'||!Number.isInteger(info.chunk_bytes)||info.chunk_bytes<1||info.chunk_bytes>48*1024)throw new Error('The selected content exceeds the web runtime limit.');
    const output=new Uint8Array(info.byte_length);let offset=0;
    while(offset<output.length){
      cancelled();
      const chunk=await bridge.request('transfer_read',{transfer_id:info.transfer_id,offset,limit:Math.min(info.chunk_bytes,48*1024)});
      cancelled();if(typeof chunk.payload_base64!=='string'||chunk.payload_base64.length>65536)throw new Error('The local content encoding exceeds its bound.');const bytes=decode(chunk.payload_base64);
      if(chunk.offset!==offset||!bytes.length||bytes.length>Math.min(info.chunk_bytes,48*1024)||bytes.length>output.length-offset||typeof chunk.finished!=='boolean')throw new Error('The local content stream returned an invalid chunk.');
      output.set(bytes,offset);offset+=bytes.length;onProgress(offset/output.length);
      if(Boolean(chunk.finished)!==(offset===output.length))throw new Error('The local content stream ended at an unexpected offset.');
    }
    if(await sha256(output.buffer)!==info.sha256)throw new Error('Content changed during transfer. The game was not started.');cancelled();
    return output.buffer;
  }catch(error){if(info?.transfer_id)await bridge.request('transfer_abort',{transfer_id:info.transfer_id}).catch(()=>{});throw error;}
}

export async function uploadSave(bridge,metadata,payload,expectedParentId){
  const bytes=new Uint8Array(payload);
  if(bytes.length!==metadata.payload_bytes||await sha256(payload)!==metadata.payload_hash)throw new Error('Save payload does not match its metadata.');
  const transfer=await bridge.request('save_upload_start',{metadata,expected_parent_id:expectedParentId});let offset=0;
  try{
    while(offset<bytes.length){
      const end=Math.min(bytes.length,offset+Math.min(transfer.chunk_bytes,48*1024));
      const result=await bridge.request('save_upload_chunk',{transfer_id:transfer.transfer_id,offset,payload_base64:encode(bytes.subarray(offset,end))});
      if(result.next_offset!==end)throw new Error('Save transfer offset is inconsistent.');offset=end;
    }
    return await bridge.request('save_upload_commit',{transfer_id:transfer.transfer_id});
  }catch(error){await bridge.request('transfer_abort',{transfer_id:transfer.transfer_id}).catch(()=>{});throw error;}
}

// The destination must be selected from the initial click, before pausing or
// capturing a potentially large snapshot consumes browser user activation.
export async function exportLiveSave(bridge,metadata,payload,grant){
  const bytes=new Uint8Array(payload);let transfer=null;
  try{
    if(!bytes.length||bytes.length>256*1024*1024||bytes.length!==metadata.payload_bytes||await sha256(payload)!==metadata.payload_hash)throw new Error('Live export bytes do not match their metadata.');
    transfer=await bridge.request('save_export_upload_start',{metadata,destination_grant_id:grant.grant_id});let offset=0;
    while(offset<bytes.length){const end=Math.min(bytes.length,offset+Math.min(transfer.chunk_bytes,48*1024));const response=await bridge.request('save_export_upload_chunk',{transfer_id:transfer.transfer_id,offset,payload_base64:encode(bytes.subarray(offset,end))});if(response.next_offset!==end)throw new Error('Live export offset is inconsistent.');offset=end;}
    return await bridge.request('save_export_upload_commit',{transfer_id:transfer.transfer_id,destination_grant_id:grant.grant_id},120000);
  }catch(error){if(transfer)await bridge.request('transfer_abort',{transfer_id:transfer.transfer_id}).catch(()=>{});throw error;}
  finally{await bridge.request('permission_revoke',{grant_id:grant.grant_id}).catch(()=>{});}
}
