"""Author APK assets and a revision-bound Chromium Android patch. No build/test."""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import shutil
from game_hub_assets import collect,prepare
from streaming_config import configuration

ROOT=Path(__file__).resolve().parents[1]
UPSTREAM=ROOT/'browser/android-snapshots'
OUTPUT=ROOT/'out/browser-android-integration'
MODULE=OUTPUT/'chrome/android/omniplay'

def replace(text,old,new):
    if text.count(old)!=1:raise ValueError('Pinned Chromium Android source anchor changed: '+old[:90])
    return text.replace(old,new,1)

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--without-cores',action='store_true');args=parser.parse_args()
    revision=json.loads((ROOT/'browser/upstream.lock.json').read_text(encoding='utf-8'))['revision']
    inventory=json.loads((UPSTREAM/'inventory.json').read_text(encoding='utf-8'))
    if inventory['revision']!=revision:raise ValueError('Android source snapshots belong to another Chromium revision')
    sources,cores=collect(args.without_cores)
    assets=MODULE/'assets/omniplay';prepare(assets,set(sources)|{'cores/catalog.json','asset-index.json'})
    for relative,path in sources.items():
        target=assets/relative;target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(path,target)
    catalog=assets/'cores/catalog.json';catalog.parent.mkdir(parents=True,exist_ok=True)
    catalog.write_text(json.dumps({'schema_version':1,'cores':cores,'development_only':args.without_cores},indent=2)+'\n',encoding='utf-8',newline='\n')
    routes=set(sources)|{'cores/catalog.json'}
    types={'.html':'text/html; charset=utf-8','.css':'text/css; charset=utf-8','.js':'application/javascript; charset=utf-8','.json':'application/json; charset=utf-8','.wasm':'application/wasm','.txt':'text/plain; charset=utf-8'}
    streaming=configuration()
    (assets/'asset-index.json').write_text(json.dumps({'schema_version':1,'streaming_origins':streaming['relay_origins'] if streaming['enabled'] else [],'assets':[{'path':p,'bytes':(assets/p).stat().st_size,'mime':types.get(Path(p).suffix,'application/octet-stream')} for p in sorted(routes)]},indent=2)+'\n',encoding='utf-8',newline='\n')
    for name in ['java','native','res']:
        shutil.copytree(ROOT/'apps/android'/name,MODULE/name,dirs_exist_ok=True)
    shutil.copy2(ROOT/'apps/android/proguard.flags',MODULE/'proguard.flags')
    (MODULE/'include').mkdir(parents=True,exist_ok=True)
    shutil.copy2(ROOT/'platform/mobile/include/omniplay_mobile.h',MODULE/'include/omniplay_mobile.h')
    paths=sorted(routes|{'asset-index.json'})
    asset_sources=',\n'.join('    '+json.dumps('assets/omniplay/'+p) for p in paths)
    asset_names=',\n'.join('    '+json.dumps('omniplay/'+p) for p in paths)
    gn=f'''# Authored by tools/assemble_android.py. Rust static archive is built explicitly.
import("//build/config/android/rules.gni")
android_assets("assets") {{
  disable_compression = true
  renaming_sources = [\n{asset_sources},\n  ]
  renaming_destinations = [\n{asset_names},\n  ]
}}
shared_library("omniplay_mobile") {{
  assert(current_cpu == "arm64", "This initial Android native artifact recipe is arm64 only")
  sources = [ "native/mobile_jni.cc" ]
  include_dirs = [ "include" ]
  inputs = [ "native/libomniplay_mobile.a" ]
  libs = [ rebase_path("native/libomniplay_mobile.a", root_build_dir), "dl", "m", "log" ]
  ldflags = [ "-Wl,--exclude-libs,ALL", "-Wl,-z,max-page-size=16384" ]
}}
'''
    (MODULE/'BUILD.gn').write_text(gn,encoding='utf-8',newline='\n')
    changes={}
    build='chrome/android/BUILD.gn';text=(UPSTREAM/build).read_text(encoding='utf-8')
    text=replace(text,'    sources = chrome_java_resources\n','    sources = chrome_java_resources\n    sources += [ "omniplay/res/values/omniplay.xml", "omniplay/res/values-vi/omniplay.xml" ]\n')
    java=sorted((ROOT/'apps/android/java').rglob('*.java'))
    additions='\n    # OmniPlay uses this Chromium renderer and the shared local Rust host.\n    sources += [\n'+''.join('      '+json.dumps('omniplay/java/'+p.relative_to(ROOT/'apps/android/java').as_posix())+',\n' for p in java)+'    ]\n    deps += [ "//chrome/android/omniplay:assets" ]\n'
    text=replace(text,'    sources = chrome_java_sources\n','    sources = chrome_java_sources\n'+additions);changes[build]=text
    apk='chrome/android/chrome_public_apk_tmpl.gni';text=(UPSTREAM/apk).read_text(encoding='utf-8')
    text=replace(text,'    shared_libraries = []\n','    shared_libraries = []\n    if (!_is_test && !_is_trichrome) {\n      shared_libraries += [ "//chrome/android/omniplay:omniplay_mobile" ]\n    }\n')
    text=replace(text,'      proguard_configs = [ "//chrome/android/proguard/main.flags" ]\n','      proguard_configs = [ "//chrome/android/proguard/main.flags", "//chrome/android/omniplay/proguard.flags" ]\n');changes[apk]=text
    activity='chrome/android/java/src/org/chromium/chrome/browser/ChromeTabbedActivity.java';text=(UPSTREAM/activity).read_text(encoding='utf-8')
    text=replace(text,'import android.app.Activity;\n','import android.app.Activity;\nimport org.chromium.chrome.browser.omniplay.GameHubController;\n')
    anchor='        if (id == R.id.new_tab_menu_id) {\n'
    text=replace(text,anchor,'        if (id == R.id.omniplay_game_hub_menu_id) {\n            if (mOmniPlayHub == null || mOmniPlayHub.isClosed()) mOmniPlayHub = new GameHubController(this);\n            mOmniPlayHub.open();\n            return true;\n        }\n'+anchor)
    text=replace(text,'    public void onPauseWithNative() {\n','    public void onPauseWithNative() {\n        if (mOmniPlayHub != null) mOmniPlayHub.onActivityPause();\n')
    text=replace(text,'    public void onResumeWithNative() {\n','    public void onResumeWithNative() {\n        if (mOmniPlayHub != null) mOmniPlayHub.onActivityResume();\n')
    text=replace(text,'    public void onDestroyInternal() {\n','    public void onDestroyInternal() {\n        if (mOmniPlayHub != null) { mOmniPlayHub.close(); mOmniPlayHub = null; }\n')
    # Field beside the unique native initialization entry point, at class scope.
    text=replace(text,'    @Override\n    public void finishNativeInitialization() {\n','    private @Nullable GameHubController mOmniPlayHub;\n\n    @Override\n    public void finishNativeInitialization() {\n');changes[activity]=text
    menu='chrome/android/java/src/org/chromium/chrome/browser/app/appmenu/AppMenuPropertiesDelegateImpl.java';text=(UPSTREAM/menu).read_text(encoding='utf-8')
    addition='''        Tab omniPlayTab = mActivityTabProvider.get();
        if (org.chromium.base.ContextUtils.activityFromContext(mContext)
                        instanceof org.chromium.chrome.browser.ChromeTabbedActivity
                && (omniPlayTab == null || !omniPlayTab.isIncognito())) {
            mModelList.add(AppMenuItemUtils.createStandardListItem(
                    AppMenuItemUtils.buildModelForStandardMenuItem(mContext, mAppMenuItemTheme,
                            R.id.omniplay_game_hub_menu_id, R.string.omniplay_game_hub, 0,
                            isMenuIconAtStart()), false));
        }
'''
    text=replace(text,'        mModelList = buildMenuModelList();\n','        mModelList = buildMenuModelList();\n'+addition);changes[menu]=text
    activity_base='chrome/android/java/src/org/chromium/chrome/browser/app/ChromeActivity.java';text=(UPSTREAM/activity_base).read_text(encoding='utf-8')
    text=replace(text,'    public void onStartWithNative() {\n','    public void onStartWithNative() {\n        org.chromium.chrome.browser.omniplay.NativeCore.setApplicationForeground(this, true);\n')
    text=replace(text,'    public void onStopWithNative() {\n','    public void onStopWithNative() {\n        org.chromium.chrome.browser.omniplay.NativeCore.setApplicationForeground(this, false);\n')
    text=replace(text,'    protected void onDestroyInternal() {\n','    protected void onDestroyInternal() {\n        org.chromium.chrome.browser.omniplay.NativeCore.setApplicationForeground(this, false);\n');changes[activity_base]=text
    manifest='chrome/android/java/AndroidManifest.xml';text=(UPSTREAM/manifest).read_text(encoding='utf-8')
    service='''        <!-- Bound only by Android's scheduler, with no renderer-accessible intent. -->
        <service android:name="org.chromium.chrome.browser.omniplay.BackgroundSyncService"
            android:permission="android.permission.BIND_JOB_SERVICE" android:exported="true"
            android:directBootAware="false" />
'''
    text=replace(text,'    </application>\n',service+'    </application>\n');changes[manifest]=text
    patch=[]
    for relative,updated in changes.items():
        original=(UPSTREAM/relative).read_text(encoding='utf-8')
        patch.extend(difflib.unified_diff(original.splitlines(keepends=True),updated.splitlines(keepends=True),fromfile='a/'+relative,tofile='b/'+relative))
        target=OUTPUT/relative;target.parent.mkdir(parents=True,exist_ok=True);target.write_text(updated,encoding='utf-8',newline='\n')
    (ROOT/'browser/patches/0002-omniplay-android.patch').write_text(''.join(patch),encoding='utf-8',newline='\n')
    (ROOT/'browser/patches/android-series.json').write_text(json.dumps({'schema_version':1,'upstream_revision':revision,'patches':[{'file':'0002-omniplay-android.patch','owner':'android','reason':'Explicit Game Hub tab with origin-bound port, SAF and shared Rust service'}],'modified_files':sorted(changes),'development_only':args.without_cores},indent=2)+'\n',encoding='utf-8',newline='\n')
    (ROOT/'apps/android/assembly-receipt.json').write_text(json.dumps({'schema_version':1,'development_only':args.without_cores,'core_ids':[c['id'] for c in cores],'resource_files':len(routes),'supported_initial_abi':['arm64-v8a'],'rust_build':'not_started','chromium_android_build':'not_started','verification':'not_run'},indent=2)+'\n',encoding='utf-8',newline='\n')
    print('Authored Chromium Android module, APK asset inventory and source patch. No build or test was started.')
if __name__=='__main__':main()
