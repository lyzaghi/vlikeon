"""Prepare or explicitly build finite pinned decoder libraries, never run media.

Source preparation invokes only Git and source/inventory authoring. Configure,
compiler and make invocations exist only behind the explicit build mode.
"""
import argparse
import json
from pathlib import Path
import shlex
import shutil
import subprocess
from ffmpeg_source import PIN,VERSION,LIBRARIES,prepare,configure_policy
from pinned_core_source import ROOT,owned,file_hash,source_inventory

RECIPES=('build_ffmpeg.py','ffmpeg_source.py','pinned_core_source.py')


def main():
    parser=argparse.ArgumentParser(description=__doc__);mode=parser.add_mutually_exclusive_group(required=True);mode.add_argument('--prepare-only',action='store_true');mode.add_argument('--build-prepared',action='store_true')
    parser.add_argument('--target',choices=['web','windows-x86_64','windows-arm64'],required=True);parser.add_argument('--git',default='git')
    for name in ('shell','make','cc','cxx','ar','ranlib','nm','ld'):parser.add_argument('--'+name,type=Path)
    parser.add_argument('--parallel',type=int,default=8);args=parser.parse_args()
    if not 1<=args.parallel<=32:raise ValueError('Build parallelism exceeds its bound')
    work=owned(ROOT/'out/decoder-sources'/('ffmpeg-'+args.target+'-9.0.2'))
    if args.prepare_only:
        _,record=prepare(work,args.git);shutil.copy2(ROOT/'security/ffmpeg-source.lock.json',work/'ffmpeg-source.lock.json')
        for name in RECIPES:shutil.copy2(ROOT/'tools'/name,work/name)
        record.update(target=args.target,source_only=True,configure='not_started',build='not_started',
            recipe_sha256={name:file_hash(work/name) for name in RECIPES},lock_sha256=file_hash(work/'ffmpeg-source.lock.json'))
        (work/'corresponding-source.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
        print('Authored pinned decoder source and finite configuration. No configure, compiler, media or test was started.');return
    required=('shell','make','cc','cxx','ar','ranlib','nm')+(('ld',) if args.target!='web' else ())
    if any(getattr(args,name) is None or not getattr(args,name).is_file() for name in required):raise ValueError('Supply explicit actual build tool paths')
    for name in required:setattr(args,name,getattr(args,name).resolve(strict=True))
    record=json.loads((work/'corresponding-source.json').read_text(encoding='utf-8'))
    if record.get('revision')!=PIN or record.get('version')!=VERSION or record.get('target')!=args.target or record.get('source_only') is not True or source_inventory(work/'source')!=record['prepared_source_sha256']:raise ValueError('The prepared decoder source changed')
    if file_hash(work/'ffmpeg-source.lock.json')!=record['lock_sha256'] or file_hash(ROOT/'security/ffmpeg-source.lock.json')!=record['lock_sha256']:raise ValueError('The selected decoder lock changed')
    for name,expected in record['recipe_sha256'].items():
        if file_hash(work/name)!=expected or file_hash(ROOT/'tools'/name)!=expected:raise ValueError('The prepared decoder recipe changed')
    build=owned(work/'build')
    if build.exists():raise ValueError('Preserve the previous decoder build and use a fresh preparation')
    source=build/'source';shutil.copytree(work/'source',source);object_dir=build/'objects';object_dir.mkdir();prefix=build/'product'
    tool=lambda path:shlex.quote(path.as_posix())
    configuration=[args.shell.as_posix(),(source/'configure').as_posix(),'--prefix='+prefix.as_posix(),*configure_policy(),
        '--cc='+tool(args.cc),'--cxx='+tool(args.cxx),'--ar='+tool(args.ar),'--ranlib='+tool(args.ranlib),'--nm='+tool(args.nm),'--pkg-config=false']
    if args.target=='web':configuration+=['--target-os=none','--arch=wasm32','--cpu=generic','--enable-cross-compile','--enable-pthreads','--disable-w32threads','--disable-os2threads',
        '--extra-cflags=-O3 -pthread -fexceptions','--extra-ldflags=-pthread']
    else:
        architecture='x86_64' if args.target=='windows-x86_64' else 'aarch64'
        triple='x86_64-pc-windows-msvc' if architecture=='x86_64' else 'aarch64-pc-windows-msvc'
        configuration+=['--target-os=win32','--arch='+architecture,'--cpu=generic','--toolchain=msvc','--enable-cross-compile','--ld='+tool(args.ld),
            '--enable-w32threads','--disable-pthreads','--disable-os2threads','--extra-cflags=-MT --target='+triple]
    subprocess.run(configuration,cwd=object_dir,check=True)
    subprocess.run([str(args.make),'-j'+str(args.parallel)],cwd=object_dir,check=True)
    subprocess.run([str(args.make),'install-libs','install-headers'],cwd=object_dir,check=True)
    libraries={name:str((prefix/'lib'/('lib'+name+'.a' if args.target=='web' else name+'.lib')).resolve()) for name in LIBRARIES}
    if any(not Path(path).is_file() for path in libraries.values()):raise ValueError('The compiler did not emit the declared static decoder libraries')
    shutil.copy2(work/'ffmpeg-source.lock.json',prefix/'ffmpeg-source.lock.json')
    for path in (work/'source').glob('COPYING*'):shutil.copy2(path,prefix/path.name)
    shutil.copy2(work/'source/LICENSE.md',prefix/'LICENSE.ffmpeg.md')
    files=source_inventory(prefix)
    receipt=dict(record,source_only=False,prefix=str(prefix.resolve()),libraries=libraries,files_sha256=files,configuration=configuration,
        tool_sha256={name:file_hash(getattr(args,name)) for name in required},runtime_crt='msvc_static' if args.target!='web' else None,
        compiled_source_sha256=source_inventory(source),verification='not_run',distribution_approved=False)
    (work/'build-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print('Built unapproved static decoder libraries. No media, application, device or test was run.')


def admit_receipt(path,target):
    """Build-time admission only; a source receipt never substitutes for binaries."""
    if path.stat().st_size>8*1024**2:raise ValueError('The decoder build receipt exceeds its bound')
    value=json.loads(path.read_text(encoding='utf-8'))
    if value.get('revision')!=PIN or value.get('version')!=VERSION or value.get('target')!=target or value.get('source_only') is not False or value.get('distribution_approved') is not False:
        raise ValueError('A compiled current decoder receipt for this exact target is required')
    prefix=Path(value['prefix']).resolve(strict=True)
    if prefix.is_symlink() or not prefix.is_dir() or source_inventory(prefix)!=value.get('files_sha256'):raise ValueError('The selected decoder product changed after its build receipt')
    expected={name:str((prefix/'lib'/('lib'+name+'.a' if target=='web' else name+'.lib')).resolve()) for name in LIBRARIES}
    if value.get('libraries')!=expected or target!='web' and value.get('runtime_crt')!='msvc_static':raise ValueError('The decoder library/CRT identity does not match this core')
    return value,prefix


if __name__=='__main__':main()
