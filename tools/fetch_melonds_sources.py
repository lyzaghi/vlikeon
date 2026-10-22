"""Materialize immutable NDS dependency sources only; never configure/build/run."""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def run(values,cwd=None):subprocess.run([str(v) for v in values],cwd=cwd,check=True)
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--only',action='append');args=parser.parse_args()
    lock=json.loads((ROOT/'security/melonds-source.lock.json').read_text(encoding='utf-8'))
    selected=[entry for entry in lock['dependencies'] if not args.only or entry['name'] in args.only]
    if args.only and set(args.only)-{entry['name'] for entry in selected}:raise ValueError('Requested dependency is absent from the immutable source lock')
    for entry in selected:
        name,revision,repository=entry['name'],entry['revision'],entry['repository']
        if not re.fullmatch('[A-Za-z0-9_-]{1,64}',name) or not re.fullmatch('[0-9a-f]{40}',revision) or not re.fullmatch(r'https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\.git',repository):raise ValueError('Dependency lock fields are not valid source identities')
        path=ROOT/'third_party/sources/melonds-deps'/name
        if (path/'.git').is_dir():
            if subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()!=revision:raise ValueError('Existing dependency checkout differs from its pin: '+name)
            print('Retained pinned source: '+name,flush=True);continue
        if path.exists() and any(path.iterdir()):raise ValueError('Source destination already contains unrelated files: '+name)
        path.mkdir(parents=True,exist_ok=True)
        run(['git','init','--quiet',path]);run(['git','-C',path,'remote','add','origin',repository])
        run(['git','-C',path,'fetch','--quiet','--depth','1','origin',revision])
        run(['git','-C',path,'-c','advice.detachedHead=false','checkout','--quiet','--detach','FETCH_HEAD'])
        if subprocess.check_output(['git','-C',str(path),'rev-parse','HEAD'],text=True).strip()!=revision:raise ValueError('Fetched source identity differs from its pin')
        print('Materialized pinned source: '+name,flush=True)
    print('Source materialization finished; no build or test was started.')
if __name__=='__main__':main()
