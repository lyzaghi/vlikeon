"""Rebuild canonical development resources twice from a disposable source copy."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess

GENERATORS=['generate_browser_patch.py','assemble_android.py','assemble_ios.py','assemble_web.py']
GENERATED=['browser/patches/0001-omniplay-game-hub.patch','browser/patches/series.json',
 'browser/patches/0002-omniplay-android.patch','browser/patches/android-series.json',
 'apps/android/assembly-receipt.json','apps/ios/assembly-receipt.json','apps/web/assembly-receipt.json',
 'apps/ios/OmniPlay.xcodeproj/project.pbxproj','apps/ios/OmniPlay.xcodeproj/xcshareddata/xcschemes/OmniPlay.xcscheme']

def inventory(base):
    result={}
    for relative in [*GENERATED,'out/browser-integration','out/browser-android-integration','apps/ios/GameHub','out/web']:
        path=base/relative
        files=[path] if path.is_file() else sorted(p for p in path.rglob('*') if p.is_file())
        for file in files:result[file.relative_to(base).as_posix()]=hashlib.sha256(file.read_bytes()).hexdigest()
    return result

def imports(base):
    problems=[];total=0;gated=[]
    for folder in ['out/browser-integration/chrome/browser/resources/omniplay_game_hub',
                   'out/browser-android-integration/chrome/android/omniplay/assets/omniplay','apps/ios/GameHub','out/web']:
        directory=base/folder
        for source in directory.rglob('*.js'):
            # Relative literal module specifiers, excluding dynamic computed URLs.
            for specifier in re.findall(r'(?:\bfrom\s*|\bimport\s*\(\s*|\bimport\s*)[\'"]([^\'"\n]+)[\'"]',source.read_text(encoding='utf-8')):
                if not specifier.startswith('.'):continue
                target=source.parent/specifier.split('?')[0].split('#')[0];total+=1
                if not target.resolve().is_relative_to(directory.resolve()) or not target.is_file():
                    # The shared bridge's Web-only dynamic branch is deliberately
                    # absent from native bundles. The Web product must resolve it.
                    if source.name=='bridge.js' and specifier=='./web/platform.js' and folder!='out/web':
                        text=source.read_text(encoding='utf-8')
                        if "?.content==='web'" in text and 'if(this.#web)return connectWeb()' in text:
                            gated.append(source.relative_to(base).as_posix());continue
                    problems.append({'file':source.relative_to(base).as_posix(),'import':specifier})
    if problems:raise ValueError('Generated module imports are missing: '+json.dumps(problems))
    return {'resolved_or_platform_gated':total,'web_only_native_exclusions':gated}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--source',type=Path,required=True);parser.add_argument('--work',type=Path,required=True)
    parser.add_argument('--python',required=True);args=parser.parse_args()
    if args.work.exists() or args.work.resolve().is_relative_to(args.source.resolve()):raise ValueError('Choose a fresh disposable sibling copy')
    shutil.copytree(args.source,args.work)
    rounds=[]
    for _ in range(2):
        for name in GENERATORS:subprocess.run([args.python,'-B',str(args.work/'tools'/name),'--without-cores'],cwd=args.work,check=True,timeout=90)
        series=json.loads((args.work/'browser/patches/series.json').read_text(encoding='utf-8'))
        if [entry['file'] for entry in series['patches']]!=['0001-omniplay-game-hub.patch','0003-owned-game-capture.patch']:raise ValueError('Owned capture patch lost during source integration')
        picker=args.work/'out/browser-integration/chrome/browser/extensions/api/desktop_capture/desktop_capture_api.cc'
        if 'SetOmniplayOwnedTarget' not in picker.read_text(encoding='utf-8'):raise ValueError('Owned capture overlay was not composed')
        imports(args.work);rounds.append(inventory(args.work))
    if rounds[0]!=rounds[1]:raise ValueError('Development source generation is not repeatable')
    changed=[name for name in GENERATED if (args.source/name).read_bytes()!=(args.work/name).read_bytes()]
    print(json.dumps({'repeatable':True,'generated_files':len(rounds[0]),'relative_imports':imports(args.work),'stale_generated_sources':changed}))
    if changed:raise ValueError('Committed generated sources need regeneration from canonical checkout bytes')

if __name__=='__main__':main()
