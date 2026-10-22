"""Final-pass only: malformed native control frames against the actual worker."""
import argparse
import json
import os
from pathlib import Path
import queue
import struct
from run_cases import Worker,digest

def header(value):return json.dumps(value,separators=(',',':')).encode()
def request(method='hello',identity=2,params=None):return {'protocol':1,'id':identity,'method':method,'params':params or {}}
def prefix(raw):return struct.pack('<I',len(raw))+raw

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker',type=Path,required=True);parser.add_argument('--core',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    if os.name!='nt':raise ValueError('Windows qualification only')
    worker_path=args.worker.resolve(strict=True);core=args.core.resolve(strict=True)
    if not worker_path.is_file() or not core.is_file():raise ValueError('Choose explicit qualification artifacts')
    output=args.output.absolute()
    if output.exists():raise ValueError('Choose a fresh retained qualification output')
    output.mkdir(parents=True)
    simple=header(request());deep=header(request(params={'value':[[[[0]]]]}))
    deep=deep.replace(b'[[[[0]]]]',b'['*64+b'0'+b']'*64)
    cases=[
        ('unknown-method',prefix(header(request('unknown'))),False,1),
        ('zero-id',prefix(header(request(identity=0))),False,1),
        ('skipped-id',prefix(header(request(identity=3))),False,1),
        ('oversized-id',prefix(header(request(identity=1<<63))),False,1),
        ('boolean-protocol',prefix(header({**request(),'protocol':True})),False,1),
        ('extra-header-key',prefix(header({**request(),'extra':0})),False,1),
        ('duplicate-header-key',prefix(b'{"protocol":1,"id":2,"id":2,"method":"hello","params":{}}'),False,1),
        ('duplicate-parameter-key',prefix(b'{"protocol":1,"id":2,"method":"hello","params":{"x":1,"x":2}}'),False,1),
        ('excessive-depth',prefix(deep),False,1),
        ('oversized-header',struct.pack('<I',65537),False,1),
        ('empty-header',prefix(b''),False,1),
        ('forbidden-payload',prefix(simple)+struct.pack('<I',0xffffffff),False,1),
        ('oversized-load-payload',prefix(header(request('load')))+struct.pack('<I',256*1024*1024+1),False,1),
        ('truncated-prefix',b'\x05\x00',True,1),
        ('truncated-header',struct.pack('<I',len(simple)+1)+simple,True,1),
        ('truncated-body-length',prefix(simple)+b'\x00\x00',True,1),
        ('truncated-load-body',prefix(header(request('load')))+struct.pack('<I',16)+b'xx',True,1),
        ('replayed-id-after-hello',prefix(header(request(identity=1))),False,1),
        ('clean-boundary-eof',b'',True,0),
    ]
    report={'verification':'running','worker_sha256':digest(worker_path),'core_sha256':digest(core),'cases':[],
            'scope':'actual direct Windows worker/qualification DLL and pipes with dummy SDL drivers; no broker/AppContainer/physical-memory proof'}
    try:
        for name,raw,close_input,expected in cases:
            worker=Worker(worker_path,core,output/name);row={'name':name,'passed':False};report['cases'].append(row)
            try:
                hello,_=worker.okay('hello')
                if hello.get('runtime_id')!='native-lifecycle-fixture' or hello.get('library_name')!='OmniPlay lifecycle qualification' or hello.get('library_version')!='1.0.0' or hello.get('pause_barrier_protocol')!=1:raise AssertionError('wrong or unhealthy qualification worker/core')
                worker.process.stdin.write(raw);worker.process.stdin.flush()
                if close_input:worker.process.stdin.close()
                # Most cases deliberately leave stdin open and omit a body
                # length. A parser that reads binary first must time out here.
                row['exit_code']=worker.process.wait(timeout=5)
                if row['exit_code']!=expected:raise AssertionError('wrong protocol terminal status')
                worker.reader.join(timeout=1)
                if worker.reader.is_alive():raise AssertionError('reply reader did not reach EOF')
                while True:
                    try:reply=worker.replies.get_nowait()
                    except queue.Empty:break
                    if not isinstance(reply,EOFError):raise AssertionError('malformed input emitted a reply or failed non-EOF framing')
                row['passed']=True
            finally:worker.close()
        report['verification']='passed'
    except BaseException:
        report['verification']='failed';raise
    finally:(output/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')

if __name__=='__main__':main()
