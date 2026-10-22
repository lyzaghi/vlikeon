"""Author/build the selected N64 GLideN64/HLE profile; no automatic execution."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
from build_mupen64plus import PIN,GL_VERSION,prepare

ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--prepare-only',action='store_true')
    for name in ['make','cc','cxx','ar']:parser.add_argument('--'+name,type=Path)
    parser.add_argument('--parallel',type=int,default=8);args=parser.parse_args()
    if not 1<=args.parallel<=32 or not args.prepare_only and any(getattr(args,name) is None for name in ['make','cc','cxx','ar']):raise ValueError('Choose explicit build tools and bounded parallelism')
    original=ROOT/'third_party/sources/mupen64plus-next'
    if subprocess.check_output(['git','-C',str(original),'rev-parse','HEAD'],text=True).strip()!=PIN:raise ValueError('N64 source differs from its immutable pin')
    output=ROOT/('out/native-sources/mupen64plus-gl' if args.prepare_only else 'out/native-cores/mupen64plus-gl')
    if output.exists():raise ValueError('Use a fresh GL profile output directory')
    source=output/'source';changes=prepare(original,source,'opengl')
    for file in [Path(__file__),ROOT/'tools/build_mupen64plus.py']:shutil.copy2(file,output/file.name)
    if args.prepare_only:
        (output/'source-preparation.json').write_text(json.dumps({'schema_version':1,'core':'mupen64plus-gl','revision':PIN,'core_version':GL_VERSION,'corresponding_source':str(source.resolve()),'adaptations':changes,'configure':'not_started','build':'not_started','verification':'not_run'},indent=2)+'\n',encoding='utf-8');print('Authored N64 GLideN64/HLE source, excluding Angrylion/CXD4/parallel directories. No compiler, runtime or test was started.');return
    # Empty WITH_DYNAREC overrides the upstream platform default. These normal
    # make variables are command-line assignments, so platform defaults cannot
    # silently restore excluded parallel, Angrylion or CXD4 build paths.
    command=[args.make,'-f','Makefile','platform=win','ARCH=x86_64','MSYSTEM=MINGW64','WITH_DYNAREC=','HAVE_THR_AL=0','LLE=0','HAVE_PARALLEL_RSP=0','HAVE_PARALLEL_RDP=0','HAVE_NEON=0','HAVE_LTCG=0','GLFLAGS=-DCORE -DHAVE_OPENGL','SYSTEM_ZLIB=0','SYSTEM_MINIZIP=0','SYSTEM_LIBPNG=0','SYSTEM_XXHASH=0','DEBUG=0','GIT_VERSION=" unknown"','CC='+str(args.cc),'CXX='+str(args.cxx),'AR='+str(args.ar),'-j'+str(args.parallel)]
    subprocess.run([str(value) for value in command],cwd=source,check=True)
    library=source/'mupen64plus_next_libretro.dll'
    if not library.is_file():raise ValueError('The selected GL library was not produced')
    notices=[source/'mupen64plus-core/LICENSES',source/'mupen64plus-core/doc/gpl-license',source/'mupen64plus-core/doc/lgpl-license',source/'mupen64plus-rsp-hle/LICENSES',source/'GLideN64/LICENSE']
    receipt={'schema_version':1,'core':'mupen64plus-gl','revision':PIN,'library':str(library.resolve()),'corresponding_source':str(source.resolve()),'additional_notices':[str(path.resolve()) for path in notices],'video_backend':'opengl_core','firmware':[],'verification':'not_run','distribution_review_required':['compiled_gliden64_hle_dependency_license_inventory','complete_corresponding_source_and_content']}
    (output/'build-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8');print('N64 GLideN64/HLE library compiled; driver, gameplay, state, containment and license verification remain pending.')
if __name__=='__main__':main()
