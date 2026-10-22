"""Fetch the locked official decoder source without building or running code."""
import argparse
import json
from pathlib import Path
import subprocess
from ffmpeg_source import PIN,VERSION
from pinned_core_source import ROOT,owned


def fetch(git='git'):
    lock=json.loads((ROOT/'security/ffmpeg-source.lock.json').read_text(encoding='utf-8'))
    if lock.get('revision')!=PIN or lock.get('version')!=VERSION or lock.get('repository')!='https://github.com/FFmpeg/FFmpeg.git' or lock.get('tag')!='n9.0.2':raise ValueError('The decoder source lock differs from this fetch recipe')
    destination=owned(ROOT/'third_party/sources/ffmpeg');tag='refs/tags/n9.0.2'
    def read(value):return subprocess.check_output([git,'-C',str(destination),'rev-parse',value],text=True).strip()
    def run(*values):subprocess.run([git,*map(str,values)],check=True)
    if (destination/'.git').is_dir():
        if read('HEAD')!=PIN or read(tag)!=lock['tag_object']:raise ValueError('Preserve the different existing decoder checkout; choose an explicit source migration first')
        print('Retained the pinned decoder source. No configure, compiler, media or test was started.');return
    if destination.exists() and any(destination.iterdir()):raise ValueError('The decoder source destination contains unrelated files')
    destination.mkdir(parents=True,exist_ok=True);run('init','--quiet',destination);run('-C',destination,'config','core.autocrlf','false');run('-C',destination,'remote','add','origin',lock['repository'])
    # A shallow complete tag avoids partial-clone lazy fetches when authoring
    # an offline source archive. This is source, not a prebuilt codec package.
    run('-C',destination,'fetch','--depth','1','origin',tag+':'+tag)
    if read(tag)!=lock['tag_object'] or read(tag+'^{commit}')!=PIN:raise ValueError('The fetched decoder tag/object identity changed')
    run('-C',destination,'-c','advice.detachedHead=false','checkout','--quiet','--detach',PIN)
    print('Materialized pinned decoder source. Tag signature and runtime qualification remain pending; no executable or test was run.')


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--git',default='git');args=parser.parse_args();fetch(args.git)


if __name__=='__main__':main()
