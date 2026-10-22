"""Retrieve revision-bound upstream API/source inputs, without building Chromium."""
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request

ROOT=Path(__file__).resolve().parents[1]
REV=json.loads((ROOT/'browser/upstream.lock.json').read_text(encoding='utf-8'))['revision']
FILES=[
 'chrome/android/BUILD.gn',
 'chrome/android/chrome_java_sources.gni',
 'chrome/android/java/AndroidManifest.xml',
 'chrome/android/java/src/org/chromium/chrome/browser/tab/TabBuilder.java',
 'chrome/browser/tab/java/src/org/chromium/chrome/browser/tab/Tab.java',
 'chrome/android/java/src/org/chromium/chrome/browser/ChromeTabbedActivity.java',
 'chrome/android/java/src/org/chromium/chrome/browser/app/ChromeActivity.java',
 'content/public/android/java/src/org/chromium/content_public/browser/JavascriptInjector.java',
 'content/public/android/java/src/org/chromium/content_public/browser/WebContents.java',
 'content/public/android/java/src/org/chromium/content_public/browser/WebContentsObserver.java',
 'content/public/android/java/src/org/chromium/content_public/browser/MessagePort.java',
 'content/public/android/java/src/org/chromium/content_public/browser/MessagePayload.java',
 'content/public/android/java/src/org/chromium/content_public/browser/NavigationHandle.java',
 'chrome/browser/tab/java/src/org/chromium/chrome/browser/tab/TabObserver.java',
 'chrome/android/java/src/org/chromium/chrome/browser/app/appmenu/AppMenuPropertiesDelegateImpl.java',
 'chrome/android/chrome_public_apk_tmpl.gni',
 'chrome/android/java_sources.gni',
 'ui/android/java/src/org/chromium/ui/base/ActivityWindowAndroid.java',
 'ui/android/java/src/org/chromium/ui/base/WindowAndroid.java',
 'ui/android/java/src/org/chromium/ui/base/IntentRequestTracker.java',
 'chrome/browser/BUILD.gn',
 'chrome/browser/android/browsing_data/browsing_data_bridge.cc',
 'chrome/browser/profiles/profile.h',
 'chrome/browser/profiles/android/java/src/org/chromium/chrome/browser/profiles/Profile.java',
 'content/public/browser/storage_partition.h',
 'storage/browser/file_system/async_file_util.h',
 'storage/browser/file_system/file_system_context.h',
 'storage/browser/file_system/file_system_operation.h',
 'storage/browser/file_system/file_system_operation_runner.h',
 'storage/browser/file_system/file_system_url.h',
 'storage/browser/file_system/open_file_system_mode.h',
 'components/services/filesystem/public/mojom/types.mojom',
 'chrome/browser/profiles/BUILD.gn',
]
def main():
 inventory={}
 for relative in FILES:
  line=subprocess.check_output(['git','-C',str(ROOT/'browser/chromium'),'ls-tree',REV,'--',relative],text=True).strip()
  if not line:raise ValueError('Pinned upstream path does not exist: '+relative)
  expected=line.split()[2];target=ROOT/'browser/android-snapshots'/relative
  def blob(data):return hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()
  if target.is_file() and blob(target.read_bytes())==expected:inventory[relative]=expected;continue
  errors=[]
  for url,encoded in [(f'https://raw.githubusercontent.com/chromium/chromium/{REV}/{relative}',False),(f'https://chromium.googlesource.com/chromium/src/+/{REV}/{relative}?format=TEXT',True)]:
   try:
    request=urllib.request.Request(url,headers={'User-Agent':'OmniPlaySourceBootstrap/1'})
    with urllib.request.urlopen(request,timeout=45) as response:data=response.read(4*1024*1024+1)
    if encoded:data=base64.b64decode(data,validate=True)
    if len(data)>4*1024*1024 or blob(data)!=expected:raise ValueError('Pinned blob identity mismatch')
    target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data);inventory[relative]=expected;print(relative,flush=True);break
   except Exception as error:errors.append(type(error).__name__)
  else:raise ValueError('Could not retrieve '+relative+': '+','.join(errors))
 (ROOT/'browser/android-snapshots/inventory.json').write_text(json.dumps({'revision':REV,'blobs':inventory},indent=2)+'\n',encoding='utf-8')
if __name__=='__main__':main()
