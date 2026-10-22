import{download}from'./save-client.js';
const UUID=/^[a-f0-9-]{36}$/;
export function checkMediaCancelled(signal){if(signal?.aborted)throw Object.assign(new Error('Game preparation was cancelled.'),{code:'cancelled'});}
async function openAsset(bridge,game,plan,asset,signal){
  checkMediaCancelled(signal);const info=await bridge.request('asset_read_start',{game_id:game.id,asset_name:plan.legacy?null:asset.name});
  try{checkMediaCancelled(signal);if(!info||typeof info.transfer_id!=='string'||!UUID.test(info.transfer_id)||info.byte_length!==asset.byte_length||info.sha256!==asset.content_hash||!Number.isInteger(info.chunk_bytes)||info.chunk_bytes<1||info.chunk_bytes>48*1024)throw new Error('The asset stream differs from its media manifest.');return info;}
  catch(error){if(info?.transfer_id)await bridge.request('transfer_abort',{transfer_id:info.transfer_id}).catch(()=>{});throw error;}
}
export async function downloadGameRom(bridge,game,plan,options={}){
  if(plan.media.assets.length!==1||plan.staged)throw new Error('This game needs file-backed media.');
  const asset=plan.media.assets[0],info=await openAsset(bridge,game,plan,asset,options.signal);return download(bridge,info,options);
}
export async function streamGameMedia(bridge,game,plan,writer,{signal,onProgress=()=>{}}={}){
  let copied=0,index=0;
  for(const asset of plan.media.assets){
    checkMediaCancelled(signal);const info=await openAsset(bridge,game,plan,asset,signal);let offset=0;
    try{
      await writer.start(asset.name);checkMediaCancelled(signal);
      while(offset<asset.byte_length){
        const limit=Math.min(info.chunk_bytes,48*1024,asset.byte_length-offset),chunk=await bridge.request('transfer_read',{transfer_id:info.transfer_id,offset,limit});
        checkMediaCancelled(signal);if(!chunk||chunk.offset!==offset||typeof chunk.payload_base64!=='string'||chunk.payload_base64.length>65536||chunk.payload_base64.length%4||!/^[A-Za-z0-9+/]*={0,2}$/.test(chunk.payload_base64)||typeof chunk.finished!=='boolean')throw new Error('The media transfer chunk is invalid.');
        const binary=atob(chunk.payload_base64),bytes=new Uint8Array(binary.length);for(let at=0;at<binary.length;at++)bytes[at]=binary.charCodeAt(at);
        if(!bytes.length||bytes.length>limit||chunk.finished!==(offset+bytes.length===asset.byte_length))throw new Error('The media transfer length is inconsistent.');
        const end=offset+bytes.length,result=await writer.chunk(asset.name,offset,bytes.buffer);checkMediaCancelled(signal);if(result.next_offset!==end)throw new Error('The media staging acknowledgement is inconsistent.');
        copied+=end-offset;offset=end;onProgress({copied_bytes:copied,total_bytes:plan.total,asset_index:index+1,asset_count:plan.media.assets.length});
      }
      await writer.finish(asset.name);index++;
    }finally{await bridge.request('transfer_abort',{transfer_id:info.transfer_id}).catch(()=>{});}
  }
}
