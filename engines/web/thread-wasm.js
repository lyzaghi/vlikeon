// Inspect the memory import before instantiation. The engine still validates the
// complete module. A shared buffer's current byteLength cannot reveal its limit.
export function validateThreadWasm(binary,policy){
  const invalid=()=>{throw new Error('The threaded emulator memory declaration is unsupported.');};
  if(!(binary instanceof ArrayBuffer)||binary.byteLength<8||binary.byteLength>512*1024*1024)invalid();
  const bytes=new Uint8Array(binary),magic=[0,97,115,109,1,0,0,0];if(magic.some((value,index)=>bytes[index]!==value))invalid();
  let offset=8,end=bytes.length,memory=null,sectionCount=0,nameBytes=0;const sections=new Set(),decoder=new TextDecoder('utf-8',{fatal:true});
  function byte(){if(offset>=end)invalid();return bytes[offset++];}
  function number(){
    let value=0,factor=1;
    for(let i=0;i<5;i++){const part=byte();if(i===4&&(part&0xf0))invalid();value+=(part&127)*factor;if(!(part&128))return value;factor*=128;}invalid();
  }
  function name(){const length=number();nameBytes+=length;if(length>1024||length>end-offset||nameBytes>1024*1024)invalid();let value;try{value=decoder.decode(bytes.subarray(offset,offset+length));}catch{invalid();}offset+=length;return value;}
  function table(){if(![0x70,0x6f].includes(byte()))invalid();const flags=number();if(flags>1)invalid();const minimum=number();if(flags&&number()<minimum)invalid();}
  while(offset<bytes.length){
    end=bytes.length;const id=byte(),size=number();if(++sectionCount>256||id>13||size>end-offset||id!==0&&sections.has(id))invalid();if(id)sections.add(id);
    const sectionEnd=offset+size;end=sectionEnd;
    if(id===2){
      const count=number();if(count>4096)invalid();
      for(let index=0;index<count;index++){
        const module=name(),field=name(),kind=byte();
        if(kind===0)number();
        else if(kind===1)table();
        else if(kind===2){
          const flags=number();if(flags!==3||memory||module!=='env'||field!=='memory')invalid();
          const minimum=number(),maximum=number();if(!minimum||maximum<minimum||maximum>policy.maximum_memory_bytes/65536)invalid();
          memory={minimum_bytes:minimum*65536,maximum_bytes:maximum*65536};
        }else if(kind===3){if(![0x7f,0x7e,0x7d,0x7c,0x7b,0x70,0x6f].includes(byte())||byte()>1)invalid();}
        else if(kind===4){if(byte()!==0)invalid();number();}
        else invalid();
      }
      if(offset!==sectionEnd)invalid();
    }else if(id===5){if(number()!==0||offset!==sectionEnd)invalid();}
    offset=sectionEnd;
  }
  if(!memory)invalid();return Object.freeze(memory);
}
