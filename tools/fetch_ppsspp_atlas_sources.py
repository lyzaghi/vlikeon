"""Materialize immutable open-font/FreeType inputs for the PSP atlas generator."""
import hashlib
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[1]
CORE='a5275067b62433375bbde12921e96329a795dff0'
SOURCES=[
    {'name':'noto-cjk','repository':'https://github.com/notofonts/noto-cjk.git','revision':'f8d157532fbfaeda587e826d4cd5b21a49186f7c','files':['Sans/LICENSE','Sans/OTF/Japanese/NotoSansCJKjp-Regular.otf']},
    {'name':'noto-fonts','repository':'https://github.com/notofonts/noto-fonts.git','revision':'ffebf8c1ee449e544955a7e813c54f9b73848eac','files':['LICENSE','hinted/ttf/NotoSans/NotoSans-Regular.ttf','hinted/ttf/NotoSansHebrew/NotoSansHebrew-Regular.ttf','hinted/ttf/NotoSansThai/NotoSansThai-Regular.ttf']},
    {'name':'freetype','repository':'https://github.com/hrydgard/freetype.git','revision':'063ed27f481f0bcd3962fd468e1546739d7d285a'},
]

def git(path,*values):return subprocess.check_output(['git','-C',str(path),*values],text=True).strip()
def run(values):subprocess.run([str(value) for value in values],check=True)
def main():
    parent=ROOT/'third_party/sources/ppsspp'
    if git(parent,'rev-parse','HEAD')!=CORE:raise ValueError('Atlas parent source differs from its pin')
    if git(parent,'ls-tree',CORE,'--','ext/freetype').split()[2]!=SOURCES[2]['revision']:raise ValueError('FreeType source differs from the parent gitlink')
    records=[]
    for entry in SOURCES:
        path=ROOT/'third_party/sources/ppsspp-atlas-deps'/entry['name']
        if not (path/'.git').is_dir():
            if path.exists() and any(path.iterdir()):raise ValueError('Atlas dependency output contains unrelated files')
            path.mkdir(parents=True,exist_ok=True);run(['git','init','--quiet',path]);run(['git','-C',path,'remote','add','origin',entry['repository']])
            fetch=['git','-C',path,'fetch','--quiet','--depth','1']
            if 'files' in entry:
                run(['git','-C',path,'config','remote.origin.promisor','true']);run(['git','-C',path,'config','remote.origin.partialclonefilter','blob:none'])
                run(['git','-C',path,'sparse-checkout','init','--no-cone']);run(['git','-C',path,'sparse-checkout','set','--no-cone',*['/'+name for name in entry['files']]])
                fetch+=['--filter=blob:none']
            run([*fetch,'origin',entry['revision']]);run(['git','-C',path,'-c','advice.detachedHead=false','checkout','--quiet','--detach','FETCH_HEAD'])
        if git(path,'rev-parse','HEAD')!=entry['revision']:raise ValueError('Atlas source differs from its immutable pin')
        record=dict(entry);record['checkout_path']=path.relative_to(ROOT).as_posix()
        if 'files' in entry:
            inventory=[]
            for relative in entry['files']:
                blob=git(path,'ls-tree',entry['revision'],'--',relative).split()
                spec=entry['revision']+':'+relative
                if len(blob)!=4 or blob[:2]!=['100644','blob']:raise ValueError('The selected font/notice is not a regular committed blob')
                size=int(git(path,'cat-file','-s',spec))
                if not 0<size<=32*1024*1024:raise ValueError('The committed font/notice exceeds its bound')
                value=subprocess.check_output(['git','-C',str(path),'show',spec])
                if len(value)!=size:raise ValueError('The committed font/notice was not materialized completely')
                inventory.append({'path':relative,'blob':blob[2],'sha256':hashlib.sha256(value).hexdigest(),'bytes':size})
            record['inventory']=inventory
            record['inventory_encoding']='raw_committed_git_blob'
        records.append(record);print('Locked immutable atlas input: '+entry['name'],flush=True)
    value={'schema_version':2,'core_revision':CORE,'sources':records,'font_license':'OFL-1.1; full pinned raw-blob notices retained','freetype_license':'FTL or GPL-2.0-or-later; selected FTL notices retained with build-tool provenance','verification':'not_run'}
    (ROOT/'security/ppsspp-atlas-source.lock.json').write_text(json.dumps(value,indent=2)+'\n',encoding='utf-8')
    print('Atlas source inputs materialized. No compiler, generator, runtime or test was started.')
if __name__=='__main__':main()
