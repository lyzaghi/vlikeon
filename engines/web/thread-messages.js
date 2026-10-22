// Only bounded SDK control messages cross the UI relay. Shared memory/modules
// are structured-cloned handles; ordinary buffers have a separate small bound.
export function threadPayload(value,maximumMemory){
  const seen=new Set(),buffers=new Set(),shared=new Set();let nodes=0,ordinaryBytes=0,sharedBytes=0;
  const invalid=()=>{throw new Error('The emulator thread control message is outside its bounds.');};
  function visit(item,depth){
    if(++nodes>256||depth>8)invalid();
    if(item===null||item===undefined||typeof item==='boolean')return;
    if(typeof item==='number'){if(!Number.isFinite(item))invalid();return;}
    if(typeof item==='bigint'){if(item<-(1n<<63n)||item>=(1n<<64n))invalid();return;}
    if(typeof item==='string'){if(item.length>16384)invalid();return;}
    if(typeof item!=='object')invalid();if(seen.has(item))return;seen.add(item);
    if(item instanceof WebAssembly.Module)return;
    if(item instanceof WebAssembly.Memory){if(!(item.buffer instanceof SharedArrayBuffer))invalid();visit(item.buffer,depth+1);return;}
    if(item instanceof SharedArrayBuffer){if(!shared.has(item)){shared.add(item);sharedBytes+=item.byteLength;}if(shared.size>4||item.byteLength>maximumMemory||sharedBytes>maximumMemory+4*1024*1024)invalid();return;}
    if(item instanceof ArrayBuffer){if(!buffers.has(item)){buffers.add(item);ordinaryBytes+=item.byteLength;}if(buffers.size>8||ordinaryBytes>1024*1024)invalid();return;}
    if(ArrayBuffer.isView(item)){visit(item.buffer,depth+1);return;}
    if(Array.isArray(item)){if(item.length>64)invalid();for(const child of item)visit(child,depth+1);return;}
    if(Object.getPrototypeOf(item)!==Object.prototype&&Object.getPrototypeOf(item)!==null)invalid();
    const keys=Object.keys(item);if(keys.length>64)invalid();for(const key of keys){if(key.length>128)invalid();visit(item[key],depth+1);}
  }
  const command=Number.isInteger(value?.cmd)&&value.cmd>=1&&value.cmd<=9;
  if(!value||typeof value!=='object'||Array.isArray(value)||!command&&!['cmd','target'].some(key=>typeof value[key]==='string'&&/^[A-Za-z][A-Za-z0-9_-]{0,63}$/.test(value[key])))invalid();
  visit(value,0);return[...buffers];
}
