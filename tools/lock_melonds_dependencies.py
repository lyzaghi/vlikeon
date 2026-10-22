"""Resolve upstream NDS build inputs to immutable commits; no configure/build/test."""
import concurrent.futures
import json
from pathlib import Path
import re
import urllib.parse
import urllib.request

ROOT=Path(__file__).resolve().parents[1]
PIN='f394adbacb5722ee97c1b37c8064da9a25818310'
def resolve(item):
    name,repository,ref=item
    parsed=urllib.parse.urlsplit(repository)
    if parsed.scheme!='https' or parsed.netloc!='github.com':raise ValueError('Expected an explicit upstream GitHub source URL')
    repo=parsed.path.strip('/').removesuffix('.git')
    endpoint='https://api.github.com/repos/'+repo+'/commits/'+urllib.parse.quote(ref,safe='')
    request=urllib.request.Request(endpoint,headers={'User-Agent':'OmniPlaySourceLock/1','Accept':'application/vnd.github+json'})
    with urllib.request.urlopen(request,timeout=45) as response:raw=response.read(3*1024*1024+1)
    if len(raw)>3*1024*1024:raise ValueError('Commit response exceeds the source metadata bound')
    revision=json.loads(raw)['sha']
    if not re.fullmatch('[0-9a-f]{40}',revision):raise ValueError('Upstream did not return a commit identity')
    return {'name':name,'repository':repository+'.git','upstream_ref':ref,'revision':revision}
def main():
    import subprocess
    source=ROOT/'third_party/sources/melonds-ds'
    if subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()!=PIN:raise ValueError('NDS parent checkout differs from the source pin')
    cmake=(source/'cmake/FetchDependencies.cmake').read_text(encoding='utf-8')
    matches=re.findall(r'fetch_dependency\(\s*"?([A-Za-z0-9_-]+)"?\s+"(https://github.com/[^\"]+)"\s+"([^\"]+)"\s*\)',cmake)
    if len(matches)!=12:raise ValueError('Pinned NDS dependency declarations changed')
    # Networking and tracing are deliberately absent from this software adapter.
    selected=[item for item in matches if item[0] not in {'libslirp','tracy'}]
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:dependencies=list(pool.map(resolve,selected))
    value={'schema_version':1,'core':{'name':'melonds-ds','repository':'https://github.com/JesseTG/melonds-ds.git','revision':PIN},'dependencies':dependencies,'excluded_build_features':['jit','opengl','compute_renderer','networking','tracy'],'verification':'not_run'}
    (ROOT/'security/melonds-source.lock.json').write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')
    print('Pinned '+str(len(dependencies))+' NDS source dependencies; no build or test was started.')
if __name__=='__main__':main()
