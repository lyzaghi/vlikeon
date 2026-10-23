"""Parse first-party source languages without executing application code."""
import argparse
import ast
import json
from pathlib import Path
import plistlib
import subprocess

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--source',type=Path,required=True);parser.add_argument('--node',required=True);args=parser.parse_args()
    counts={'python':0,'javascript':0,'json':0,'plist':0}
    for file in sorted(args.source.rglob('*')):
        if not file.is_file() or file.relative_to(args.source).parts[0] in {'branding','third_party'}:continue
        if file.suffix=='.py':ast.parse(file.read_text(encoding='utf-8-sig'),filename=str(file));counts['python']+=1
        elif file.suffix in {'.js','.mjs'}:
            result=subprocess.run([args.node,'--input-type=module','--check'],input=file.read_bytes(),capture_output=True,timeout=15)
            if result.returncode:raise ValueError(str(file.relative_to(args.source))+': '+result.stderr.decode(errors='replace'))
            counts['javascript']+=1
        elif file.suffix=='.json':
            text=file.read_text(encoding='utf-8-sig')
            # Chromium's pinned extension schema has a copyright line-comment
            # preamble. Strip full comment lines only for that reference file;
            # URLs/string contents and all product JSON remain strict JSON.
            if file.relative_to(args.source).as_posix()=='browser/upstream-snapshots/chrome/common/extensions/api/desktop_capture.json':
                text='\n'.join(line for line in text.splitlines() if not line.lstrip().startswith('//'))
            try:json.loads(text)
            except ValueError as error:raise ValueError(str(file.relative_to(args.source))+': '+str(error)) from error
            counts['json']+=1
        elif file.suffix in {'.plist','.xcprivacy','.entitlements'}:plistlib.loads(file.read_bytes());counts['plist']+=1
    print(json.dumps({'parsed':counts}))
if __name__=='__main__':main()
