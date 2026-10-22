import{WebPlatformError,integer,requireValue,HASH}from'./errors.js';

export const SYNC_CRYPTO_JSON=1024*1024,SYNC_CRYPTO_BINARY=256*1024*1024+64*1024+32;
export const SYNC_CRYPTO_WASM=16*1024*1024;
export const SYNC_CRYPTO_EXPORTS=Object.freeze(['op_alloc','op_free','op_command','op_result_ptr','op_result_len','op_binary_ptr','op_binary_len','op_clear_result','op_clear','op_protocol']);
export const SYNC_WASI_IMPORTS=Object.freeze(['random_get','proc_exit','fd_write','fd_read','fd_close','fd_seek','fd_fdstat_get','environ_sizes_get','environ_get','args_sizes_get','args_get','clock_time_get']);
const exact=(value,keys)=>value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===keys.length&&keys.every(key=>Object.hasOwn(value,key));
export function syncCryptoConfiguration(value){
  requireValue(exact(value,['schema_version','enabled','protocol','wasm_path','wasm_bytes','wasm_sha256','distribution_approved'])&&value.schema_version===1&&typeof value.enabled==='boolean'&&value.protocol===1&&typeof value.distribution_approved==='boolean','untrusted_artifact','The browser synchronization crypto configuration is invalid.');
  if(!value.enabled){requireValue(value.wasm_path===null&&value.wasm_bytes===0&&value.wasm_sha256===null&&!value.distribution_approved,'untrusted_artifact','Disabled browser crypto cannot reference an executable.');return null;}
  requireValue(value.distribution_approved&&value.wasm_path==='sync-crypto.wasm'&&integer(value.wasm_bytes,8,SYNC_CRYPTO_WASM)&&typeof value.wasm_sha256==='string'&&HASH.test(value.wasm_sha256),'untrusted_artifact','Browser crypto needs an explicitly admitted local artifact.');return{...value};
}

class Reader{
  constructor(bytes,start=0,end=bytes.length){this.bytes=bytes;this.at=start;this.end=end;}
  byte(){requireValue(this.at<this.end,'untrusted_artifact','The crypto module is truncated.');return this.bytes[this.at++];}
  uint(){let value=0,shift=0;for(let count=0;count<5;count++){const byte=this.byte();requireValue(count!==4||(byte&240)===0,'untrusted_artifact','The crypto module integer overflows.');value+=(byte&127)*2**shift;if(!(byte&128))return value;shift+=7;}throw new WebPlatformError('untrusted_artifact','The crypto module integer is unterminated.');}
  name(){const length=this.uint();requireValue(length<=256&&this.at+length<=this.end,'untrusted_artifact','The crypto module name exceeds its bound.');let value;try{value=new TextDecoder('utf-8',{fatal:true}).decode(this.bytes.subarray(this.at,this.at+length));}catch{throw new WebPlatformError('untrusted_artifact','The crypto module name is not UTF-8.');}this.at+=length;return value;}
  done(){requireValue(this.at===this.end,'untrusted_artifact','The crypto module section has trailing data.');}
}
export function admitSyncWasm(bytes){
  requireValue(bytes instanceof Uint8Array&&integer(bytes.length,8,SYNC_CRYPTO_WASM)&&[0,97,115,109,1,0,0,0].every((value,index)=>bytes[index]===value),'untrusted_artifact','The crypto artifact is not a bounded WebAssembly module.');
  const reader=new Reader(bytes,8),seen=new Set(),imports=[],exports=[];let memory=false;
  while(reader.at<reader.end){const kind=reader.byte(),length=reader.uint(),end=reader.at+length;requireValue(kind<=12&&end<=reader.end&&!seen.has(kind)&&kind!==8,'untrusted_artifact','The crypto module uses an unsupported section or automatic start.');if(kind)seen.add(kind);const section=new Reader(bytes,reader.at,end);
    if(kind===2){const count=section.uint();requireValue(count<=SYNC_WASI_IMPORTS.length,'untrusted_artifact','The crypto module import budget is exceeded.');for(let index=0;index<count;index++){const module=section.name(),name=section.name(),type=section.byte();requireValue(module==='wasi_snapshot_preview1'&&SYNC_WASI_IMPORTS.includes(name)&&type===0&&!imports.includes(name),'untrusted_artifact','The crypto module requests ambient host authority.');section.uint();imports.push(name);}section.done();}
    if(kind===5){requireValue(section.uint()===1&&section.uint()===1&&section.uint()===512&&section.uint()===16384,'untrusted_artifact','Browser crypto requires one bounded private 32-bit memory.');section.done();memory=true;}
    if(kind===7){const count=section.uint();requireValue(count>=SYNC_CRYPTO_EXPORTS.length+1&&count<=SYNC_CRYPTO_EXPORTS.length+5,'untrusted_artifact','The crypto module export budget is invalid.');for(let index=0;index<count;index++){const name=section.name(),type=section.byte();section.uint();requireValue(!exports.some(item=>item.name===name),'untrusted_artifact','The crypto module repeats an export.');const admitted=type===0&&(SYNC_CRYPTO_EXPORTS.includes(name)||name==='_initialize')||type===2&&name==='memory'||type===3&&['__heap_base','__data_end'].includes(name)||type===1&&name==='__indirect_function_table';requireValue(admitted,'untrusted_artifact','The crypto module exposes an unsupported host interface.');exports.push({name,type});}section.done();}
    reader.at=end;
  }
  requireValue(memory&&imports.includes('random_get')&&SYNC_CRYPTO_EXPORTS.every(name=>exports.some(item=>item.name===name&&item.type===0))&&exports.some(item=>item.name==='memory'&&item.type===2),'untrusted_artifact','The crypto module is missing its finite ABI or secure randomness import.');return{imports,exports};
}

// No network, filesystem, preopened directory, stdout or environmental authority
// is provided. Unsupported operations return an error; randomness has no fallback.
export function syncWasi(memory){
  const span=(pointer,length,max=SYNC_CRYPTO_JSON)=>{pointer>>>=0;length>>>=0;const buffer=memory().buffer;requireValue(length<=max&&pointer<=buffer.byteLength&&length<=buffer.byteLength-pointer,'untrusted_artifact','The crypto module supplied an invalid memory span.');return new Uint8Array(buffer,pointer,length);};
  const zero=pointer=>{const bytes=span(pointer,4);new DataView(bytes.buffer,bytes.byteOffset,4).setUint32(0,0,true);};
  return{
    random_get:(pointer,length)=>{const target=span(pointer,length);requireValue(typeof crypto?.getRandomValues==='function','unsupported_capability','Secure browser randomness is unavailable.');for(let offset=0;offset<target.length;offset+=65536)crypto.getRandomValues(target.subarray(offset,Math.min(target.length,offset+65536)));return 0;},
    proc_exit:()=>{throw new WebPlatformError('runtime_crashed','The browser crypto module stopped.');},
    fd_write:()=>8,fd_read:()=>8,fd_close:()=>8,fd_seek:()=>8,fd_fdstat_get:()=>8,
    environ_sizes_get:(count,size)=>{zero(count);zero(size);return 0;},environ_get:()=>0,
    args_sizes_get:(count,size)=>{zero(count);zero(size);return 0;},args_get:()=>0,
    clock_time_get:()=>58,
  };
}
