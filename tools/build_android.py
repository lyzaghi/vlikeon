"""Explicit Linux Chromium/Android build recipe. Do not invoke during authoring."""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['cargo','gn','ninja','ndk']:parser.add_argument('--'+name,type=Path,required=True)
    parser.add_argument('--api',type=int,required=True,help='API level matching the selected Chromium minimum SDK')
    parser.add_argument('--bundle',action='store_true',help='Also author the Android App Bundle using Chromium tooling')
    parser.add_argument('--development',action='store_true',help='Explicitly allow an empty development core catalog')
    args=parser.parse_args()
    if platform.system()!='Linux':raise ValueError('Chromium Android builds require the selected Linux build host and toolchain')
    if not 29<=args.api<=99:raise ValueError('Select an explicit supported API level, at least 29')
    receipt=json.loads((ROOT/'apps/android/assembly-receipt.json').read_text(encoding='utf-8'))
    if not args.development and (receipt['development_only'] or not receipt['core_ids']):raise ValueError('Release builds require approved bundled cores and release assembly')
    checkout=ROOT/'browser/chromium';module=checkout/'chrome/android/omniplay'
    if not (module/'assets/omniplay/asset-index.json').is_file():raise ValueError('Stage the authored Android integration first')
    toolchain=args.ndk/'toolchains/llvm/prebuilt/linux-x86_64/bin'
    compiler=toolchain/f'aarch64-linux-android{args.api}-clang';archiver=toolchain/'llvm-ar'
    if not compiler.is_file() or not archiver.is_file():raise ValueError('The explicitly selected NDK lacks the requested compiler/API level')
    environment=os.environ.copy();environment.update({'CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER':str(compiler),'CC_aarch64_linux_android':str(compiler),'AR_aarch64_linux_android':str(archiver),'CARGO_TARGET_DIR':str(ROOT/'target')})
    subprocess.run([str(args.cargo),'build','--locked','--release','--target','aarch64-linux-android','-p','omniplay-mobile'],cwd=ROOT,env=environment,check=True)
    shutil.copy2(ROOT/'target/aarch64-linux-android/release/libomniplay_mobile.a',module/'native/libomniplay_mobile.a')
    config=(ROOT/'browser/config/android-release.gn').read_text(encoding='utf-8')
    if args.development:config=config.replace('is_official_build = true','is_official_build = false')
    destination=checkout/'out/OmniPlayAndroid';destination.mkdir(parents=True,exist_ok=True)
    existing=destination/'args.gn'
    if existing.is_file() and existing.read_text(encoding='utf-8')!=config:raise ValueError('The selected build directory contains different GN arguments; choose a fresh explicit build directory')
    existing.write_text(config,encoding='utf-8')
    subprocess.run([str(args.gn),'gen',str(destination)],cwd=checkout,check=True)
    targets=['chrome_public_apk']+(['chrome_public_bundle'] if args.bundle else [])
    subprocess.run([str(args.ninja),'-C',str(destination)]+targets,cwd=checkout,check=True)
    print('Android compilation completed. Artifact signing, store identity and real-device verification are separate release steps.')
if __name__=='__main__':main()
