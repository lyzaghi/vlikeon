"""Lock/materialize selected PSP gitlink sources; no configuration or execution."""
import argparse
import configparser
import json
from pathlib import Path
import re
import subprocess

ROOT=Path(__file__).resolve().parents[1]
PIN='a5275067b62433375bbde12921e96329a795dff0'
SELECTED=['libretro/libretro-common','ffmpeg','ext/armips','ext/glslang','ext/SPIRV-Cross','ext/rapidjson','ext/zstd','ext/cpu_features','ext/rcheevos','ext/libchdr','ext/lua','ext/nanosvg','ext/aemu_postoffice']

def revision(source):return subprocess.check_output(['git','-C',str(source),'rev-parse','HEAD'],text=True).strip()
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--lock-only',action='store_true');parser.add_argument('--include-parent-ffmpeg',action='store_true',help='Also retain the obsolete parent gitlink for source archaeology; current product recipes still require the modern decoder');args=parser.parse_args()
    parent=ROOT/'third_party/sources/ppsspp'
    if revision(parent)!=PIN:raise ValueError('PSP parent differs from its pinned source')
    modules=configparser.ConfigParser();modules.read(parent/'.gitmodules',encoding='utf-8')
    urls={modules[section]['path']:modules[section]['url'] for section in modules.sections()}
    dependencies=[]
    for relative in SELECTED:
        metadata=subprocess.check_output(['git','-C',str(parent),'ls-tree',PIN,'--',relative],text=True).strip().split()
        if len(metadata)!=4 or metadata[:2]!=['160000','commit'] or not re.fullmatch('[0-9a-f]{40}',metadata[2]):raise ValueError('Selected PSP dependency is not an immutable gitlink')
        repository=urls[relative].removesuffix('.git')+'.git'
        if not re.fullmatch(r'https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\.git',repository):raise ValueError('PSP dependency URL is outside the selected source provider')
        name=relative.replace('/','-');dependencies.append({'name':name,'path':relative,'repository':repository,'revision':metadata[2]})
    lock={'schema_version':1,'core':{'name':'ppsspp','repository':'https://github.com/hrydgard/ppsspp.git','revision':PIN},'dependencies':dependencies,'excluded_optional_modules':['pspautotests','frametests','assets/debugger','ext/discord-rpc','ext/miniupnp','ext/freetype','ext/OpenXR-SDK','ext/libadrenotools','ext/oboe','ios/MoltenVK','SDL/macOS'],'verification':'not_run'}
    destination=ROOT/'security/ppsspp-source.lock.json'
    if destination.exists() and json.loads(destination.read_text(encoding='utf-8'))!=lock:raise ValueError('Existing PSP source lock differs from the selected source graph')
    destination.write_text(json.dumps(lock,indent=2)+'\n',encoding='utf-8')
    if args.lock_only:print('Authored immutable PSP source lock; no build or test started.');return
    for entry in dependencies:
        if entry['name']=='ffmpeg' and not args.include_parent_ffmpeg:continue
        path=ROOT/'third_party/sources/ppsspp-deps'/entry['name']
        if (path/'.git').is_dir():
            if revision(path)!=entry['revision']:raise ValueError('Existing dependency differs from its gitlink: '+entry['name'])
            print('Retained pinned PSP source: '+entry['name'],flush=True);continue
        if path.exists() and any(path.iterdir()):raise ValueError('Dependency output contains unrelated files')
        path.mkdir(parents=True,exist_ok=True)
        for command in [['git','init','--quiet',path],['git','-C',path,'remote','add','origin',entry['repository']],['git','-C',path,'fetch','--quiet','--depth','1','origin',entry['revision']],['git','-C',path,'-c','advice.detachedHead=false','checkout','--quiet','--detach','FETCH_HEAD']]:subprocess.run([str(v) for v in command],check=True)
        if revision(path)!=entry['revision']:raise ValueError('Fetched PSP source identity mismatch')
        print('Materialized pinned PSP source: '+entry['name'],flush=True)
    from fetch_ffmpeg_sources import fetch as fetch_current_decoder
    fetch_current_decoder()
    print('PSP source materialization finished with the explicit modern decoder override; no configuration, compiler, runtime or test started.')
if __name__=='__main__':main()
