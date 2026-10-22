"""Stage the product component and generate a small, revision-bound patch queue.

This authors source files. It does not build or run Chromium or execute tests.
"""
from __future__ import annotations
import argparse
import base64
import difflib
import hashlib
import json
from pathlib import Path
import re
import shutil
from game_hub_assets import collect,prepare
from desktop_capture_audio_patch import author as author_capture_audio

ROOT=Path(__file__).resolve().parents[1]
SNAPSHOTS=ROOT/"browser/upstream-snapshots"
SNAPSHOT_REVISION="0e66b94549a8bd85f03fbdf189b752d6e7b63c32"

def replace_once(text,old,new,label):
    if text.count(old)!=1:
        raise SystemExit(f"Upstream integration anchor changed: {label}")
    return text.replace(old,new,1)

def main():
    parser=argparse.ArgumentParser()
    parser.add_argument("--without-cores",action="store_true",help="Author a development-only UI patch before core builds exist")
    args=parser.parse_args()
    upstream=json.loads((ROOT/"browser/upstream.lock.json").read_text(encoding="utf-8"))
    if upstream.get('revision')!=SNAPSHOT_REVISION:
        raise SystemExit('Migrate the exact upstream snapshots and patch anchors before changing the Chromium pin')
    identity=json.loads((ROOT/"security/component_identity.json").read_text(encoding="utf-8"))
    manifest=json.loads((ROOT/'apps/game-hub/manifest.json').read_text(encoding='utf-8'))
    public_key=base64.b64decode(identity['public_key_base64'],validate=True)
    fingerprint=hashlib.sha256(public_key).hexdigest()[:32]
    extension_id=''.join(chr(ord('a')+int(value,16)) for value in fingerprint)
    expected_origin='chrome-extension://'+extension_id+'/'
    if manifest.get('key')!=identity['public_key_base64'] or identity.get('extension_id')!=extension_id or identity.get('origin')!=expected_origin or (ROOT/'security/game_hub_origin.txt').read_text(encoding='utf-8').strip()!=expected_origin:
        raise SystemExit('The bundled component, allowlist and native broker must share one exact public identity')
    output=ROOT/"out/browser-integration"
    resource_output=output/"chrome/browser/resources/omniplay_game_hub"
    sources,catalog=collect(args.without_cores,extension=True)
    prepare(resource_output,set(sources)|{"cores/catalog.json"})
    catalog_path=resource_output/"cores/catalog.json"
    catalog_path.parent.mkdir(parents=True,exist_ok=True)
    catalog_path.write_text(json.dumps({"schema_version":1,"cores":catalog,"development_only":args.without_cores},indent=2)+"\n",encoding="utf-8")
    files=[]
    for relative,path in sorted(sources.items()):
        target=resource_output/relative
        target.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(path,target)
        files.append(relative)
    files.append("cores/catalog.json")
    resource_ids={}
    for relative in sorted(files):
        name="IDR_OMNIPLAY_"+("MANIFEST" if relative=="manifest.json" else re.sub(r"[^A-Z0-9]","_",relative.upper()))
        if name in resource_ids.values():
            raise SystemExit("Resource identifier collision")
        resource_ids[relative]=name
    includes="\n      <!-- OmniPlay bundled Game Hub. -->\n      <if expr=\"not is_android and not is_chromeos\">\n"
    for relative,name in resource_ids.items():
        includes+=f'        <include name="{name}" file="omniplay_game_hub/{relative}" resource_path="omniplay_game_hub/{relative}" type="BINDATA" />\n'
    includes+='      </if>\n'
    changes=author_capture_audio(SNAPSHOTS)
    loader_path="chrome/browser/extensions/component_loader.cc"
    loader=(SNAPSHOTS/loader_path).read_text(encoding="utf-8")
    anchor="  if (should_disable_background_extensions) {\n    return;\n  }\n"
    addition='''
#if BUILDFLAG(IS_WIN) || BUILDFLAG(IS_MAC) || BUILDFLAG(IS_LINUX)
  // Keep gameplay services out of private profiles and ordinary web origins.
  if (!skip_session_components && !profile_->IsOffTheRecord()) {
    Add(IDR_OMNIPLAY_MANIFEST,
        base::FilePath(FILE_PATH_LITERAL("omniplay_game_hub")));
  }
#endif
'''
    changes[loader_path]=replace_once(loader,anchor,anchor+addition,"component loader")
    allow_path="chrome/browser/extensions/component_extensions_allowlist/allowlist.cc"
    allow=(SNAPSHOTS/allow_path).read_text(encoding="utf-8")
    allow=replace_once(allow,"      extension_misc::kPdfExtensionId,\n",f'      extension_misc::kPdfExtensionId,\n#if BUILDFLAG(IS_WIN) || BUILDFLAG(IS_MAC) || BUILDFLAG(IS_LINUX)\n      "{identity["extension_id"]}",\n#endif\n',"component identity")
    allow=replace_once(allow,"    case IDR_READING_MODE_GDOCS_HELPER_MANIFEST:\n",'    case IDR_READING_MODE_GDOCS_HELPER_MANIFEST:\n#if BUILDFLAG(IS_WIN) || BUILDFLAG(IS_MAC) || BUILDFLAG(IS_LINUX)\n    case IDR_OMNIPLAY_MANIFEST:\n#endif\n',"component manifest")
    changes[allow_path]=allow
    grd_path="chrome/browser/resources/component_extension_resources.grd"
    grd=(SNAPSHOTS/grd_path).read_text(encoding="utf-8")
    changes[grd_path]=replace_once(grd,"    <includes>\n","    <includes>\n"+includes,"resource list")
    brand_path="chrome/app/theme/chromium/BRANDING"
    brand=(SNAPSHOTS/brand_path).read_text(encoding="utf-8")
    replacements={"COMPANY_FULLNAME":"OmniPlay","COMPANY_SHORTNAME":"OmniPlay","PRODUCT_FULLNAME":"OmniPlay","PRODUCT_SHORTNAME":"OmniPlay","PRODUCT_INSTALLER_FULLNAME":"OmniPlay Installer","PRODUCT_INSTALLER_SHORTNAME":"OmniPlay Installer","MAC_BUNDLE_ID":"app.omniplay.browser"}
    changes[brand_path]="\n".join(f"{line.split('=',1)[0]}={replacements[line.split('=',1)[0]]}" if '=' in line and line.split('=',1)[0] in replacements else line for line in brand.splitlines())+"\n"
    paths={
        "chrome/common/chrome_paths_linux.cc":[('std::string data_dir_basename = "chromium";','std::string data_dir_basename = "omniplay";')],
        "chrome/common/chrome_paths_mac.mm":[('product_dir_name = "Chromium";','product_dir_name = "OmniPlay";')],
        "chrome/common/chrome_paths.cc":[('/Library/Application Support/Chromium/NativeMessagingHosts','/Library/Application Support/OmniPlay/NativeMessagingHosts'),('/etc/chromium/native-messaging-hosts','/etc/omniplay/native-messaging-hosts')],
        "chrome/install_static/chromium_install_modes.h":[
            ('kProductPathName[] = L"Chromium";','kProductPathName[] = L"OmniPlay";'),
            ('.base_app_name = L"Chromium"','.base_app_name = L"OmniPlay"'),
            ('.base_app_id = L"Chromium"','.base_app_id = L"OmniPlay"'),
            ('L"ChromiumHTM"','L"OmniPlayHTM"'),('L"ChromiumPDF"','L"OmniPlayPDF"'),
            ('L"Chromium HTML Document"','L"OmniPlay HTML Document"'),('L"Chromium PDF Document"','L"OmniPlay PDF Document"'),
            ('.direct_launch_url_scheme = "chromium"','.direct_launch_url_scheme = "omniplay"'),
        ],
    }
    for relative,edits in paths.items():
        updated=(SNAPSHOTS/relative).read_text(encoding="utf-8")
        for old,new in edits:
            updated=replace_once(updated,old,new,relative)
        changes[relative]=updated
    launcher_path="chrome/browser/extensions/api/messaging/launch_context_win.cc"
    launcher=(SNAPSHOTS/launcher_path).read_text(encoding="utf-8")
    launcher_anchor='''#if BUILDFLAG(CHROMIUM_BRANDING)
  static constexpr wchar_t kChromiumNativeMessagingRegistryKey[] ='''
    launcher_addition='''#if BUILDFLAG(CHROMIUM_BRANDING)
  // OmniPlay's privileged broker is registered only in its own product hive.
  // Other extension hosts retain upstream compatibility discovery.
  if (host_name == L"com.omniplay.broker") {
    return GetManifestPathWithFlagsFromSubkey(
        root_key, flags, L"SOFTWARE\\\\OmniPlay\\\\NativeMessagingHosts",
        host_name, result);
  }
  static constexpr wchar_t kChromiumNativeMessagingRegistryKey[] ='''
    changes[launcher_path]=replace_once(launcher,launcher_anchor,launcher_addition,"OmniPlay native host registry")
    patch=[]
    for relative,updated in changes.items():
        original=(SNAPSHOTS/relative).read_text(encoding="utf-8")
        patch.extend(difflib.unified_diff(original.splitlines(keepends=True),updated.splitlines(keepends=True),fromfile=f"a/{relative}",tofile=f"b/{relative}"))
        target=output/relative
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_text(updated,encoding="utf-8",newline="\n")
    patch_dir=ROOT/"browser/patches"
    patch_dir.mkdir(parents=True,exist_ok=True)
    (patch_dir/"0001-omniplay-game-hub.patch").write_text("".join(patch),encoding="utf-8",newline="\n")
    (patch_dir/"series.json").write_text(json.dumps({"schema_version":1,"upstream_revision":upstream['revision'],"patches":[{"file":"0001-omniplay-game-hub.patch","owner":"browser","reason":"Bundle Game Hub and isolate product profiles/native host; retain upstream sandbox and browser UI"}],"modified_files":sorted(changes),"resource_files":sorted(files),"development_only":args.without_cores},indent=2)+"\n",encoding="utf-8")
    print("Authored Chromium integration patch and staged product component.")

if __name__=="__main__":
    main()
