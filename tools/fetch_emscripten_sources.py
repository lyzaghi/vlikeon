"""Materialize the explicitly pinned SDK source contracts, never an SDK binary.

No compiler, bootstrap, installer, package manager, configure or test is invoked.
Existing different checkouts are preserved instead of reset or switched.
"""
import argparse
import json
from pathlib import Path
import re
import subprocess

ROOT=Path(__file__).resolve().parents[1]
REVISION='e13ee988ce3d87a738dd8f3661d02223308c412a'
REPOSITORY='https://github.com/emscripten-core/emscripten.git'


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--git',default='git');args=parser.parse_args()
    lock=json.loads((ROOT/'security/emscripten-source.lock.json').read_text(encoding='utf-8'))
    if lock.get('schema_version')!=1 or lock.get('revision')!=REVISION or lock.get('repository')!=REPOSITORY or lock.get('version')!='6.0.13' or not re.fullmatch(r'[0-9a-f]{40}',REVISION):raise ValueError('The SDK source lock does not match this source recipe')
    destination=ROOT/'third_party/sources/emscripten'
    if destination.resolve()!=destination.absolute() or not destination.resolve().is_relative_to(ROOT.resolve()):raise ValueError('The SDK source destination must remain inside the unlinked workspace')
    def run(*values):subprocess.run([args.git,*map(str,values)],check=True)
    def current():return subprocess.check_output([args.git,'-C',str(destination),'rev-parse','HEAD'],text=True).strip()
    if (destination/'.git').is_dir():
        if current()!=REVISION:raise ValueError('Preserve the existing different SDK checkout; select an explicit source migration first')
        print('Retained the pinned SDK source checkout. No compiler, installer, runtime or test was started.');return
    if destination.exists() and any(destination.iterdir()):raise ValueError('The SDK source destination contains unrelated files')
    destination.mkdir(parents=True,exist_ok=True)
    run('init','--quiet',destination)
    run('-C',destination,'config','core.autocrlf','false')
    run('-C',destination,'remote','add','origin',REPOSITORY)
    run('-C',destination,'sparse-checkout','init','--cone')
    run('-C',destination,'sparse-checkout','set','src','system/include/emscripten','tools')
    run('-C',destination,'fetch','--depth','1','--filter=blob:none','origin',REVISION)
    run('-C',destination,'-c','advice.detachedHead=false','checkout','--quiet','--detach','FETCH_HEAD')
    if current()!=REVISION:raise ValueError('The SDK source checkout does not match its immutable revision')
    print('Materialized the pinned SDK source contracts. No compiler, installer, bootstrap, runtime or test was started.')


if __name__=='__main__':main()
