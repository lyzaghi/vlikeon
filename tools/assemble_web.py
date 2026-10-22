"""Author an inventoried Web/PWA bundle. Never start a server, browser or test."""
import argparse
import hashlib
import json
import mimetypes
import os
from pathlib import Path
import shutil
from game_hub_assets import collect,prepare,safe_relative,regular
from streaming_config import connect_sources
from web_sync_resources import resources as crypto_resources
from web_sync_network import resources as sync_resources,connect_sources as sync_connect_sources

ROOT=Path(__file__).resolve().parents[1]
SOURCE=ROOT/'apps/web'
OUTPUT=ROOT/'out/web'
MARKER='.omniplay-generated.json'
WEB_FILES=['bootstrap.js','styles.css','icon.svg','manifest.webmanifest']+['platform/'+name+'.js' for name in ['errors','idb','sha256','hash-worker','hash-client','saves','library','disc-source','disc-plan','transfers','files','firmware','export-stream','sessions','preferences','performance','storage-inventory','unused-assets','sync-wasm-policy','sync-crypto-worker','sync-crypto','sync-vault','sync-credentials','sync-network-policy','sync-http','sync-account','sync-lease','sync-storage','sync-consent','sync-save-identity','sync-hash-worker','sync-hash','sync-journal','sync-resumable','sync-recovery-state','sync-recovery','sync-upload-recovery','sync-reconcile','sync-run','platform']]
HEADERS={
    'Content-Security-Policy':"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self'; media-src 'self' blob:; manifest-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    'Cross-Origin-Opener-Policy':'same-origin','Cross-Origin-Embedder-Policy':'require-corp','Cross-Origin-Resource-Policy':'same-origin','X-Content-Type-Options':'nosniff','Referrer-Policy':'no-referrer',
    'Permissions-Policy':'camera=(), microphone=(), geolocation=(), payment=(), usb=(), serial=(), hid=()',
}
def sha(data):return hashlib.sha256(data).hexdigest()
def info(value):
    if isinstance(value,bytes):return len(value),sha(value)
    digest=hashlib.sha256();size=0
    with value.open('rb') as stream:
        for block in iter(lambda:stream.read(256*1024),b''):size+=len(block);digest.update(block)
    return size,digest.hexdigest()
def encoded(value):return (json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(',',':'))+'\n').encode('utf-8')
def previous_output():
    if not OUTPUT.exists():return
    if OUTPUT.is_symlink() or getattr(OUTPUT,'is_junction',lambda:False)():raise ValueError('Web output must be an owned unlinked directory')
    if not (OUTPUT/MARKER).is_file():
        if any(OUTPUT.iterdir()):raise ValueError('Refusing to replace an unrecognized web output directory')
        return
    receipt=json.loads(regular(OUTPUT/MARKER,OUTPUT).read_text(encoding='utf-8'));known={safe_relative(item['path']):item['sha256'] for item in receipt['files']}
    for directory,folders,files in os.walk(OUTPUT,topdown=True,followlinks=False):
        for name in folders+files:
            path=Path(directory)/name
            if path.is_symlink() or getattr(path,'is_junction',lambda:False)() or not path.resolve().is_relative_to(OUTPUT):raise ValueError('Web output contains a linked or unowned entry')
        for filename in files:
            path=Path(directory)/filename;regular(path,OUTPUT);name=path.relative_to(OUTPUT).as_posix()
            if name!=MARKER and (name not in known or info(path)[1]!=known[name]):raise ValueError('Generated web output has a local edit: '+name)

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--without-cores',action='store_true');args=parser.parse_args()
    sources,cores=collect(args.without_cores,extension=False);payload=dict(sources)
    extra=' '.join(sorted(set(connect_sources().split()+sync_connect_sources())));headers=dict(HEADERS)
    if extra:headers['Content-Security-Policy']=headers['Content-Security-Policy'].replace("connect-src 'self';","connect-src 'self' "+extra+';')
    for name in WEB_FILES:
        path=SOURCE/name;target='manifest.webmanifest' if name=='manifest.webmanifest' else 'web/'+path.name;safe_relative(target)
        if target in payload:raise ValueError('Web product resources collide with the shared Game Hub')
        payload[target]=regular(path,SOURCE)
    crypto_payload=crypto_resources(regular)
    for target,value in {**crypto_payload,**sync_resources(json.loads(crypto_payload['web/sync-crypto.json'])['enabled'])}.items():
        if target in payload:raise ValueError('Browser crypto resources collide with the shared Game Hub')
        payload[target]=value
    index=payload['index.html'].read_text(encoding='utf-8');anchor='  <link rel="stylesheet" href="styles.css">'
    if index.count(anchor)!=1:raise ValueError('Shared Game Hub document anchor changed')
    index=index.replace(anchor,anchor+'\n  <meta name="omniplay-platform" content="web">\n  <meta name="theme-color" content="#0b1218">\n  <link rel="manifest" href="manifest.webmanifest">\n  <link rel="icon" href="web/icon.svg" type="image/svg+xml">\n  <link rel="stylesheet" href="web/styles.css">\n  <script type="module" src="web/bootstrap.js"></script>')
    payload['index.html']=index.encode('utf-8');payload['cores/catalog.json']=encoded({'schema_version':1,'cores':cores})
    assets=[]
    for name,data in sorted(payload.items()):
        size,digest=info(data)
        if not 0<size<=512*1024*1024:raise ValueError('Web resource length is outside its bound: '+name)
        mime={'.js':'text/javascript; charset=utf-8','.json':'application/json','.html':'text/html; charset=utf-8','.css':'text/css; charset=utf-8','.svg':'image/svg+xml','.webmanifest':'application/manifest+json','.wasm':'application/wasm','.txt':'text/plain; charset=utf-8'}.get(Path(name).suffix,mimetypes.guess_type(name)[0] or 'application/octet-stream')
        # Large executable core payloads are integrity checked and cached on first
        # use. An app-shell install does not promise every core is offline-ready.
        precache=not(name.startswith('cores/') and Path(name).suffix in {'.wasm','.js'} or name=='web/sync-crypto.wasm')
        assets.append({'path':name,'bytes':size,'sha256':digest,'mime':mime,'precache':precache})
    worker_template=regular(SOURCE/'service-worker.js',SOURCE).read_text(encoding='utf-8')
    build_id=sha(encoded({'assets':assets,'worker_template_sha256':sha(worker_template.encode('utf-8')),'headers':headers}));manifest=encoded({'schema_version':1,'build_id':build_id,'development_only':args.without_cores,'assets':assets})
    if len(assets)>4096 or len(manifest)>1024*1024:raise ValueError('Web resource inventory exceeds the service-worker policy')
    payload['asset-manifest.json']=manifest
    worker=worker_template
    worker=worker.replace("__OMNIPLAY_CONNECT_SOURCES__",extra)
    for token,value in [('__OMNIPLAY_BUILD_ID__',build_id),('__OMNIPLAY_MANIFEST_HASH__',sha(manifest))]:
        if worker.count(token)!=1:raise ValueError('Web worker template identity anchor changed')
        worker=worker.replace(token,value)
    payload['service-worker.js']=worker.encode('utf-8');payload['deployment-headers.json']=encoded(headers)
    previous_output();prepare(OUTPUT,[*payload,MARKER])
    for name,data in payload.items():
        target=OUTPUT/name;target.parent.mkdir(parents=True,exist_ok=True)
        if isinstance(data,bytes):target.write_bytes(data)
        else:shutil.copy2(data,target)
    files=[]
    for name,data in sorted(payload.items()):
        size,digest=info(data);files.append({'path':name,'bytes':size,'sha256':digest})
    (OUTPUT/MARKER).write_bytes(encoded({'schema_version':1,'development_only':args.without_cores,'build_id':build_id,'files':files,'verification':'not_run'}))
    receipt={'schema_version':1,'build_id':build_id,'development_only':args.without_cores,'resource_files':len(files),'core_ids':[core['id'] for core in cores],'server':'not_started','browser':'not_started','verification':'not_run'}
    (SOURCE/'assembly-receipt.json').write_bytes(encoded(receipt))
    print('Authored an inventoried Web/PWA resource bundle. No server, browser, core or test was started.')
if __name__=='__main__':main()
