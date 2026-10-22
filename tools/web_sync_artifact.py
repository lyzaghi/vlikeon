"""Finite static admission for the browser crypto ABI; never execute WASM."""
import hashlib
import json

MAX_WASM=16*1024*1024
EXPORTS={'op_alloc','op_free','op_command','op_result_ptr','op_result_len','op_binary_ptr','op_binary_len','op_clear_result','op_clear','op_protocol'}
IMPORTS={'random_get','proc_exit','fd_write','fd_read','fd_close','fd_seek','fd_fdstat_get','environ_sizes_get','environ_get','args_sizes_get','args_get','clock_time_get'}

class Reader:
    def __init__(self,data,start=0,end=None):self.data=data;self.at=start;self.end=len(data) if end is None else end
    def byte(self):
        if self.at>=self.end:raise ValueError('Truncated browser crypto module')
        value=self.data[self.at];self.at+=1;return value
    def uint(self):
        value=0
        for count in range(5):
            byte=self.byte()
            if count==4 and byte&240:raise ValueError('Overflowing browser crypto integer')
            value+=(byte&127)<<(7*count)
            if not byte&128:return value
        raise ValueError('Unterminated browser crypto integer')
    def name(self):
        length=self.uint()
        if length>256 or self.at+length>self.end:raise ValueError('Unbounded browser crypto interface name')
        value=self.data[self.at:self.at+length].decode('utf-8');self.at+=length;return value
    def done(self):
        if self.at!=self.end:raise ValueError('Trailing browser crypto section data')

def inspect(data):
    if not 8<=len(data)<=MAX_WASM or data[:8]!=b'\0asm\x01\0\0\0':raise ValueError('Invalid browser crypto artifact')
    reader=Reader(data,8);seen=set();imports=[];exports={};memory=False
    while reader.at<reader.end:
        kind=reader.byte();length=reader.uint();end=reader.at+length
        if kind>12 or kind==8 or end>reader.end or kind in seen:raise ValueError('Unsupported browser crypto section/start')
        if kind:seen.add(kind)
        part=Reader(data,reader.at,end)
        if kind==2:
            count=part.uint()
            if count>len(IMPORTS):raise ValueError('Unbounded browser crypto imports')
            for _ in range(count):
                module=part.name();name=part.name();interface=part.byte()
                if module!='wasi_snapshot_preview1' or name not in IMPORTS or interface!=0 or name in imports:raise ValueError('Ambient browser crypto import rejected')
                part.uint();imports.append(name)
            part.done()
        if kind==5:
            if [part.uint() for _ in range(4)]!=[1,1,512,16384]:raise ValueError('Browser crypto needs one bounded private memory')
            part.done();memory=True
        if kind==7:
            count=part.uint()
            if not len(EXPORTS)+1<=count<=len(EXPORTS)+5:raise ValueError('Unbounded browser crypto exports')
            for _ in range(count):
                name=part.name();interface=part.byte();part.uint()
                admitted=interface==0 and name in EXPORTS|{'_initialize'} or interface==2 and name=='memory' or interface==3 and name in {'__heap_base','__data_end'} or interface==1 and name=='__indirect_function_table'
                if name in exports or not admitted:raise ValueError('Unexpected browser crypto export')
                exports[name]=interface
            part.done()
        reader.at=end
    if not memory or 'random_get' not in imports or any(exports.get(name)!=0 for name in EXPORTS) or exports.get('memory')!=2:raise ValueError('Missing finite browser crypto ABI')
    return {'imports':sorted(imports),'exports':exports,'memory_initial_pages':512,'memory_maximum_pages':16384,'automatic_start':False}

def admitted_package(directory,regular):
    """A build receipt is only a candidate; a separate policy must approve it."""
    receipt=regular(directory/'build-receipt.json',directory)
    if receipt.stat().st_size>1024*1024:raise ValueError('Browser crypto receipt exceeds its bound')
    value=json.loads(receipt.read_text(encoding='utf-8'));artifact=regular(directory/'sync-crypto.wasm',directory)
    if artifact.stat().st_size>MAX_WASM:raise ValueError('Browser crypto artifact exceeds its bound')
    data=artifact.read_bytes();policy=inspect(data)
    if value.get('source_only') is not False or value.get('target')!='wasm32-wasip1' or value.get('protocol')!=1 or value.get('wasm_bytes')!=len(data) or value.get('wasm_sha256')!=hashlib.sha256(data).hexdigest() or value.get('module_policy')!=policy:raise ValueError('Browser crypto candidate identity changed')
    return artifact,value
