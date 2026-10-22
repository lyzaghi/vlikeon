"""Explicit later build/staging recipe for the signed-release native input helper.

No tests or application runs are part of this recipe. Do not invoke during source
authoring under the current consolidated-verification instruction.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess

ROOT=Path(__file__).resolve().parents[1]
def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--cargo',required=True,type=Path);parser.add_argument('--target',default='x86_64-pc-windows-msvc',choices=['x86_64-pc-windows-msvc','aarch64-pc-windows-msvc']);args=parser.parse_args()
    if platform.system()!='Windows':raise ValueError('The authored helper backend requires an explicit Windows toolchain')
    output=ROOT/'out/stream-input'/args.target
    if output.exists():raise ValueError('Use a fresh native-input staging output')
    env=os.environ.copy();env['CARGO_TARGET_DIR']=str(output/'build');env['RUSTFLAGS']='-C target-feature=+crt-static -C link-arg=/DYNAMICBASE -C link-arg=/NXCOMPAT -C link-arg=/HIGHENTROPYVA'
    subprocess.run([str(args.cargo),'build','--locked','--release','--package','omniplay-stream-input','--bin','omniplay-input-worker','--target',args.target],cwd=ROOT,env=env,check=True)
    original=output/'build'/args.target/'release/omniplay-input-worker.exe'
    if not original.is_file() or original.is_symlink():raise ValueError('The expected input helper was not produced')
    staged=output/'package/helpers/omniplay-input-worker.exe';staged.parent.mkdir(parents=True);shutil.copy2(original,staged)
    receipt={'schema_version':1,'target':args.target,'path':'helpers/omniplay-input-worker.exe','source_package':'platform/stream-input','bytes':staged.stat().st_size,'sha256':hashlib.sha256(staged.read_bytes()).hexdigest(),'cargo_lock_sha256':hashlib.sha256((ROOT/'Cargo.lock').read_bytes()).hexdigest(),'private_protocol':1,'privilege':'normal_user_as_invoker','verification':'not_run','release_requires':'complete_signed_outer_package_inventory_and_consolidated_containment_input_device_verification'}
    (output/'build-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    print('Built and staged the input helper. It has not been run, admitted into a release or verified.')
if __name__=='__main__':main()
