"""Read-only acceptance for the exact task-3 Windows Chromium build."""
import argparse,hashlib,json,struct
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--project',type=Path,required=True);p.add_argument('--batch-result',type=Path,required=True);p.add_argument('--expected-preparation',required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args()
if a.output.exists():raise ValueError('Fresh acceptance evidence required')
root=a.project.resolve(strict=True)
def safe(relative):
 path=root/relative
 if not path.resolve().is_relative_to(root):raise ValueError('Artifact escape')
 for ancestor in [path,*path.parents]:
  if ancestor.is_symlink() or ancestor.is_junction():raise ValueError('Linked artifact')
  if ancestor==root:break
 if not path.is_file():raise ValueError('Missing artifact: '+relative)
 return path
def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as stream:
  for data in iter(lambda:stream.read(1024*1024),b''):h.update(data)
 return h.hexdigest()
preparation=safe('preparation-receipt.json')
if sha(preparation)!=a.expected_preparation:raise ValueError('Different prepared input')
receipt=json.loads(safe('build-receipt.json').read_text());journal=json.loads(safe('build-journal.json').read_text());batch=json.loads(a.batch_result.read_text())
if batch.get('status')!='passed' or not any(c.get('id')=='chromium-build' and c.get('exit_code')==0 for c in batch.get('cases',[])):raise ValueError('No successful full build exit')
if journal.get('phase')!='built' or journal.get('failed') is not None or journal.get('running') is not None:raise ValueError('Build journal not completed')
for key,value in {'target':'windows-x86_64','configuration':'completed','compiler':'completed','upstream_revision':'0e66b94549a8bd85f03fbdf189b752d6e7b63c32','preparation_sha256':a.expected_preparation,'development_only':True,'browser':'not_started','verification':'not_run','distribution_approved':False}.items():
 if receipt.get(key)!=value:raise ValueError('Unexpected build receipt field: '+key)
if receipt['binary_relative_path']!='workspace/src/out/OmniPlay/chrome.exe':raise ValueError('Unexpected browser entrypoint')
binary=safe(receipt['binary_relative_path'])
if binary.stat().st_size!=receipt['binary_bytes'] or sha(binary)!=receipt['binary_sha256']:raise ValueError('Browser entrypoint mismatch')
with binary.open('rb') as stream:
 header=stream.read(64)
 if len(header)!=64 or header[:2]!=b'MZ':raise ValueError('Not a Windows executable')
 offset=struct.unpack_from('<I',header,60)[0]
 if not 64<=offset<=1024*1024:raise ValueError('PE header position')
 stream.seek(offset);pe=stream.read(26)
 if len(pe)!=26 or pe[:4]!=b'PE\0\0' or struct.unpack_from('<H',pe,4)[0]!=0x8664 or struct.unpack_from('<H',pe,24)[0]!=0x20b:raise ValueError('Browser is not AMD64 PE32+')
if sha(safe('dependency-revisions.txt'))!=receipt['dependency_revisions_sha256']:raise ValueError('Dependency revision report changed')
source=json.loads(preparation.read_text())
for item in source['source_inventory']:
 file=safe('source/'+item['path'])
 if file.stat().st_size!=item['bytes'] or sha(file)!=item['sha256']:raise ValueError('Frozen input changed: '+item['path'])
artifacts={}
for relative in ['chrome.exe','chrome.dll','chrome_elf.dll','resources.pak','icudtl.dat','v8_context_snapshot.bin','locales/en-US.pak']:
 file=safe('workspace/src/out/OmniPlay/'+relative);artifacts[relative]={'bytes':file.stat().st_size,'sha256':sha(file)}
result={'schema_version':1,'item':3,'passed':True,'scope':'Exact development Chromium chrome build and selected output integrity only; no browser launch/device/release acceptance','receipt_sha256':sha(safe('build-receipt.json')),'preparation_sha256':sha(preparation),'artifacts':artifacts,'frozen_inputs_unchanged':True,'architecture':'AMD64 PE32+'}
a.output.parent.mkdir(parents=True,exist_ok=True);a.output.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8');print(json.dumps(result))