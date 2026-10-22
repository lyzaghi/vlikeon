"""Final-pass only: actual worker/DLL/pipe/exit qualification, no product catalog."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import struct
import subprocess
import threading
import time

CORE_NAME='omniplay-lifecycle-fixture.dll'

def exact(stream,size):
    value=bytearray()
    while len(value)<size:
        part=stream.read(size-len(value))
        if not part:raise EOFError('worker channel ended')
        value.extend(part)
    return bytes(value)

class Worker:
    def __init__(self,worker,core,directory,environment_overrides=None):
        directory.mkdir()
        shutil.copy2(worker,directory/'omniplay-native-worker.exe')
        shutil.copy2(core,directory/CORE_NAME)
        (directory/'game.bin').write_bytes(b'LIFECYCLE'+bytes(55))
        environment=os.environ.copy()
        environment.update(SDL_VIDEODRIVER='dummy',SDL_AUDIODRIVER='dummy')
        if environment_overrides:environment.update(environment_overrides)
        self.process=subprocess.Popen([str(directory/'omniplay-native-worker.exe'),'--stdio-runtime-v1'],cwd=directory,env=environment,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
        self.replies=queue.Queue(maxsize=8);self.sequence=0;self.history=[]
        self.reader=threading.Thread(target=self.read,daemon=True)
        try:self.reader.start()
        except BaseException:
            self.process.kill();self.process.wait(timeout=5)
            self.process.stdin.close();self.process.stdout.close();raise

    def read(self):
        try:
            while True:
                size=struct.unpack('<I',exact(self.process.stdout,4))[0]
                if size>65536:raise ValueError('oversized reply header')
                header=json.loads(exact(self.process.stdout,size))
                size=struct.unpack('<I',exact(self.process.stdout,4))[0]
                if size>4096:raise ValueError('fixture payload exceeds bound')
                self.replies.put((header,exact(self.process.stdout,size)),timeout=1)
        except BaseException as error:
            try:self.replies.put(error,timeout=1)
            except queue.Full:pass

    def call(self,method,params=None,payload=b''):
        self.sequence+=1
        header=json.dumps({'protocol':1,'id':self.sequence,'method':method,'params':params or {}},separators=(',',':')).encode()
        self.process.stdin.write(struct.pack('<I',len(header))+header+struct.pack('<I',len(payload))+payload);self.process.stdin.flush()
        result=self.replies.get(timeout=5)
        if isinstance(result,BaseException):raise result
        reply,payload=result
        if reply.get('protocol')!=1 or reply.get('id')!=self.sequence:raise AssertionError('wrong reply identity')
        self.history.append({'method':method,'status':reply.get('status'),'error_code':reply.get('error_code')})
        return reply,payload

    def okay(self,method,params=None):
        reply,payload=self.call(method,params)
        if reply.get('status')!='ok':raise AssertionError('unexpected rejection: '+method)
        return reply.get('result'),payload

    def prepare(self,scenario,protocol=1):
        hello,_=self.okay('hello')
        if hello.get('runtime_id')!='native-lifecycle-fixture' or hello.get('library_name')!='OmniPlay lifecycle qualification' or hello.get('pause_barrier_protocol')!=1:raise AssertionError('wrong qualification worker/core')
        return self.call('prepare',{'filename':'game.bin','title':'Lifecycle qualification','bytes':64,'core_options':{'fixture_scenario':scenario},'config':{'audio_volume':.5,'latency_frames':2,'preset':'accurate'},'input_profile':{'schema_version':1,'keyboard':[None]*16,'analog_keyboard':[None]*4,'button_sources':list(range(16))},'pause_barrier':True,'pause_barrier_protocol':protocol})

    def run_frame(self):
        self.okay('start')
        until=time.monotonic()+3
        # The first pause scenario must run at least one frame. Native status's
        # active time only becomes positive after actual core loop progress.
        while time.monotonic()<until:
            status,_=self.okay('status')
            if status.get('active_playtime_ms',0)>0:return
            time.sleep(.02)
        raise AssertionError('fixture core made no observed progress')

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
        self.process.wait(timeout=5)
        self.process.stdin.close();self.process.stdout.close();self.reader.join(timeout=1)

def digest(path):return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--worker',type=Path,required=True);parser.add_argument('--core',type=Path,required=True);parser.add_argument('--void-core',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    if os.name!='nt':raise ValueError('Windows qualification only')
    artifacts=[path.resolve(strict=True) for path in (args.worker,args.core,args.void_core)]
    for artifact in artifacts:
        if not artifact.is_file() or artifact.is_symlink():raise ValueError('Choose ordinary explicit qualification artifacts')
    output=args.output.absolute()
    if output.exists():raise ValueError('Choose a fresh retained qualification output')
    output.mkdir(parents=True)
    report={'verification':'running','artifacts':[{'path':str(path),'sha256':digest(path)} for path in artifacts],'cases':[],'scope':'actual direct Windows worker and qualification DLL with SDL dummy drivers; no broker/AppContainer/real-core/device proof'}
    try:
        for scenario in ('normal','init-throw','load-throw','run-throw','serialize-throw','unserialize-throw','pause-refuse','pause-throw','pause-invalid','unload-throw','deinit-throw','void-only','protocol-zero'):
            worker=Worker(artifacts[0],artifacts[2] if scenario=='void-only' else artifacts[1],output/scenario)
            row={'scenario':scenario,'passed':False};report['cases'].append(row)
            try:
                reply=None
                try:reply,_=worker.prepare(scenario,0 if scenario=='protocol-zero' else 1)
                except EOFError:
                    if scenario not in ('init-throw','load-throw'):raise
                    row['prepare_eof']=True
                if scenario=='protocol-zero':
                    if reply.get('status')!='error' or reply.get('error_code')!='unsupported_capability':raise AssertionError('undeclared protocol admitted')
                    worker.okay('stop');expected=0
                elif scenario in ('init-throw','load-throw'):
                    if reply is not None:raise AssertionError('unknown core exception unwound to a reply')
                    expected=1
                elif scenario=='void-only':
                    if reply.get('status')!='error':raise AssertionError('legacy export admitted')
                    expected=1
                elif scenario=='run-throw':
                    if reply.get('status')!='ok':raise AssertionError('fixture prepare failed')
                    worker.okay('start');expected=1
                else:
                    if reply.get('status')!='ok':raise AssertionError('fixture prepare failed')
                    worker.run_frame()
                    if scenario in ('serialize-throw','unserialize-throw'):
                        try:
                            if scenario=='serialize-throw':worker.call('save',{'kind':'emulator_state'})
                            else:worker.call('load',{'kind':'emulator_state'},b'OPNS0001'+struct.pack('<III',0xffffffff,0,16)+bytes(16))
                        except EOFError:pass
                        else:raise AssertionError('unknown snapshot exception unwound into a reply')
                        expected=1
                    elif scenario=='normal':
                        worker.okay('pause');_,memory=worker.okay('save',{'kind':'in_game'})
                        if len(memory)!=32 or memory[:8]!=b'OPRM0001':raise AssertionError('wrong fixture memory envelope')
                        pauses,unloads,deinits,runs=struct.unpack('<IIII',memory[16:])
                        if pauses!=2 or unloads or deinits or not runs:raise AssertionError('pause was skipped or replayed')
                        worker.okay('stop');expected=0
                    else:
                        try:
                            failure,_=worker.call('pause' if scenario.startswith('pause-') else 'stop')
                            if failure.get('status')!='error':raise AssertionError('unknown lifetime acknowledged success')
                        except EOFError:pass
                        expected=1
                row['exit_code']=worker.process.wait(timeout=5)
                if row['exit_code']!=expected:raise AssertionError('unexpected exit; 86 means forbidden callback replay or DLL/static destruction')
                row['passed']=True
            finally:
                row['calls']=worker.history;worker.close()
        report['verification']='passed'
    except BaseException:
        report['verification']='failed';raise
    finally:
        (output/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')

if __name__=='__main__':main()
