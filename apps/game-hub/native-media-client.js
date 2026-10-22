import{checkMediaCancelled}from'./media-client.js';
const UUID=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
export function nativeMediaAvailable(hello){return hello?.native_asset_ranges===1&&globalThis.crossOriginIsolated===true&&typeof SharedArrayBuffer==='function'&&typeof Atomics.wait==='function';}
export async function openNativeMedia(bridge,game,plan,{signal,onProgress=()=>{}}={}){
  let job=null,set=null,closed=false,reading=false,renewing=false,timer=0;const until=performance.now()+1800000,listeners=new Set();
  let closing=null;
  const close=()=>{if(closing)return closing;closed=true;clearInterval(timer);closing=(async()=>{if(typeof set?.set_id==='string'&&UUID.test(set.set_id))await bridge.request('asset_ranges_close',{set_id:set.set_id},10000).catch(()=>{});if(typeof job?.id==='string'&&UUID.test(job.id))await bridge.request('asset_ranges_cancel',{job_id:job.id},10000).catch(()=>{});})();return closing;};
  try{
    checkMediaCancelled(signal);job=await bridge.request('asset_ranges_start',{game_id:game.id});
    if(!job||typeof job.id!=='string'||!UUID.test(job.id)||job.game_id!==game.id)throw new Error('Native media preparation returned an invalid job.');
    const jobId=job.id;while(true){
      checkMediaCancelled(signal);if(performance.now()>=until)throw new Error('Native media verification exceeded its time limit.');
      if(job.id!==jobId||job.game_id!==game.id||!['hashing','ready','failed','cancelled','expired'].includes(job.state)||!Number.isSafeInteger(job.bytes_hashed)||job.bytes_hashed<0||job.bytes_hashed>plan.total||job.total_bytes!==plan.total||job.asset_count!==plan.media.assets.length||!Number.isInteger(job.asset_index)||job.asset_index<0||job.asset_index>job.asset_count||typeof job.result_available!=='boolean')throw new Error('Native media verification status is inconsistent.');
      onProgress(job);
      if(job.state==='ready')break;if(job.state!=='hashing')throw new Error(job.error?.message||'Native media verification did not finish.');
      await new Promise(resolve=>setTimeout(resolve,500));checkMediaCancelled(signal);job=await bridge.request('asset_ranges_status',{job_id:job.id});
    }
    set=await bridge.request('asset_ranges_collect',{job_id:job.id});checkMediaCancelled(signal);
    if(!set||typeof set.set_id!=='string'||!UUID.test(set.set_id)||set.game_id!==game.id||set.game_content_hash!==plan.content_hash||set.byte_length!==plan.total||set.read_limit!==65536||set.lease_ms!==120000||!Array.isArray(set.assets)||set.assets.length!==plan.media.assets.length)throw new Error('The native media set differs from its library identity.');
    for(let index=0;index<set.assets.length;index++){const native=set.assets[index],asset=plan.media.assets[index];if(native.index!==index||native.name!==asset.name||native.sha256!==asset.content_hash||native.byte_length!==asset.byte_length)throw new Error('A native media asset differs from its manifest.');}
    const source={
      description:Object.freeze({media:plan.media,system:plan.system,total:plan.total,content_hash:plan.content_hash,set_id:set.set_id,read_limit:65536}),
      onFailure(listener){listeners.add(listener);return()=>listeners.delete(listener);},
      async read({set_id,asset_index,offset,limit}){
        if(closed||reading||set_id!==set.set_id||!Number.isInteger(asset_index)||asset_index<0||asset_index>=set.assets.length||!Number.isSafeInteger(offset)||offset<0||!Number.isInteger(limit)||limit<=0||limit>65536)throw new Error('Native media read is closed, busy or invalid.');
        const asset=set.assets[asset_index],expected=Math.min(limit,asset.byte_length-offset);if(expected<=0)throw new Error('Native media read is outside the verified asset.');reading=true;
        try{
          const result=await bridge.request('asset_range_read',{set_id,asset_index,offset,limit},20000);
          if(closed||result.set_id!==set_id||result.asset_index!==asset_index||result.offset!==offset||typeof result.finished!=='boolean'||result.finished!==(offset+expected===asset.byte_length)||typeof result.payload_base64!=='string'||result.payload_base64.length>87384||result.payload_base64.length%4||!/^[A-Za-z0-9+/]*={0,2}$/.test(result.payload_base64))throw new Error('The native media read response is inconsistent.');
          const binary=atob(result.payload_base64);if(binary.length!==expected)throw new Error('Native media returned an incomplete positional read.');const bytes=new Uint8Array(expected);for(let index=0;index<expected;index++)bytes[index]=binary.charCodeAt(index);return bytes;
        }finally{reading=false;}
      },close
    };
    timer=setInterval(()=>{if(closed||renewing)return;renewing=true;void bridge.request('asset_ranges_keep_alive',{set_id:set.set_id},10000).catch(()=>{if(!closed)for(const listener of listeners)listener();void close();}).finally(()=>{renewing=false;});},30000);
    return Object.freeze(source);
  }catch(error){await close();throw error;}
}
