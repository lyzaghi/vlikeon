const MAX=64*1024*1024;
export function coreMemoryFloor(core){
  const value=core?.minimum_system_ram_bytes??0;
  if(!Number.isSafeInteger(value)||value<0||value>MAX)throw new Error('The installed core memory admission policy is invalid.');
  return value;
}
export function observeSystemMemory(core,minimum){
  minimum=coreMemoryFloor({minimum_system_ram_bytes:minimum});
  const size=core._retro_get_memory_size(2),pointer=core._retro_get_memory_data(2);
  if(!Number.isSafeInteger(size)||size<0||size>core.HEAPU8.length||!Number.isSafeInteger(pointer)||pointer<0||
     size>0&&(pointer===0||pointer>core.HEAPU8.length-size)||size<minimum)
    throw new Error('The emulator did not expose the required loaded-game memory.');
  return size;
}
