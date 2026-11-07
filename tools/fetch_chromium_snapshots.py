"""Fetch exact integration-source blobs while the full upstream checkout hydrates."""
from __future__ import annotations
import base64
import hashlib
from pathlib import Path
import urllib.request

ROOT=Path(__file__).resolve().parents[1]
REVISION="6449d9f9a6a906edff724b7b220199b3fde4615c"
BLOBS={
    "chrome/renderer/resources/extensions/desktop_capture_custom_bindings.js":"21848da0017d8547582281d49957a43082d81fc9",
    "chrome/browser/media/webrtc/desktop_media_picker.cc":"171bba7423f062c13dca44694a5ecaa93a62c81f",
    "content/public/browser/desktop_media_id.h":"1b9ddbef4bac9a53659d20bd774207e1e88c6fbe",
    "chrome/browser/ui/views/desktop_capture/desktop_media_picker_views.cc":"148d42eef075a0f97eba4b0116746142adb5ee74",
    "chrome/browser/ui/views/desktop_capture/desktop_media_list_controller.cc":"d53fc57c8b7a679c6738f52062f8db4cd743f010",
    "chrome/browser/media/webrtc/desktop_media_picker_utils.cc":"da55ba857ded4a1a6f01019b7dae0def8800946c",
    "chrome/browser/extensions/api/webrtc_desktop_capture_private/webrtc_desktop_capture_private_api.cc":"5d075d13df813736fe7137ba9c252e7fc8408e91",
    "chrome/browser/media/webrtc/desktop_media_picker_controller.cc":"ab4dfb7d3e578af4614e81a78bb104240adad87f",
    "chrome/browser/media/webrtc/desktop_media_picker_controller.h":"759566cdffa2fe7fa4e4643794c1b1cb64a67e46",
    "chrome/browser/media/webrtc/display_media_access_handler.cc":"addfa22ae66a8198eb645b5f62b858f589239798",
    "chrome/common/extensions/api/desktop_capture.json":"975b51070f28e8b28a2aec1d75e3c3830b1365a5",
    "third_party/blink/public/mojom/mediastream/media_stream.mojom":"ae24a58c318a2c5026c90f20eb52eb5a01ff9b48",
    "chrome/browser/extensions/api/desktop_capture/BUILD.gn":"8e374ccc08d4cc5e0d6db7435ef088eecb85375b",
    "chrome/browser/extensions/api/desktop_capture/desktop_capture_api.cc":"fe672eb9a07c499fa81fa909d1de4b562e43e56e",
    "chrome/browser/extensions/api/desktop_capture/desktop_capture_api.h":"27d79fdd63221a48c899d4cfc0ed981b8a09f8d2",
    "chrome/browser/extensions/api/desktop_capture/desktop_capture_base.cc":"eeda40174892deb448c505b4b6951f29f7c44f5e",
    "chrome/browser/extensions/api/desktop_capture/desktop_capture_base.h":"4ac6604b073dfc869e0ea7c0111c9efacb5c0b49",
    "chrome/browser/media/webrtc/desktop_media_list.h":"c6fb99a6ced5730a4a79089065d317f43a95fc3b",
    "chrome/browser/media/webrtc/desktop_media_picker.h":"8d2f2275b20c51de8439187a631d637c00a79fd1",
    "chrome/browser/media/webrtc/desktop_media_picker_factory.h":"e3e1eefeff1a822a47bb37c6b31d6a1f70eaabdd",
    "chrome/browser/media/webrtc/native_desktop_media_list.h":"dfda4956aa664d1b5aaf578f6df18570bf4daca8",
    "chrome/browser/extensions/BUILD.gn":"aa70420081ffc57b015f04839174f75b69e9ca16",
    "chrome/browser/extensions/component_loader.cc":"e6feac0151a4fa1a2aa587d27ff7bd3668982aab",
    "chrome/browser/extensions/component_loader.h":"448883bbdbaa48248bd40bcb1c0e677dc02b2415",
    "chrome/browser/resources/component_extension_resources.grd":"37f5f7edf273e7ad3651a5f83fd6c1e2892c0912",
    "chrome/browser/extensions/component_extensions_allowlist/allowlist.cc":"720671265ee9851248a0f6369f758f0872e31ca3",
    "chrome/app/theme/chromium/BRANDING":"f8363d5b294fe210ebfdd18374e95b54d6ff5c17",
    "chrome/browser/extensions/api/messaging/native_message_process_host.cc":"14562bbd56465139db43f797894a6e88f82055ab",
    "chrome/common/chrome_paths.cc":"4448f86df86f764874539303e1c002e3c85f7e66",
    "chrome/browser/extensions/api/messaging/native_process_launcher.cc":"e67315a5aa12d512c16e68c010c409c5411f515f",
    "chrome/common/chrome_constants.cc":"1a6b899b4bc3a35a53ca99d9b8b6d8cf25e7ea6a",
    "chrome/browser/extensions/api/messaging/launch_context_win.cc":"bde1be53e4cc246c55c7a036ad6c84edb520fe0a",
    "chrome/common/chrome_paths_linux.cc":"5575c049b222f52b529ded1a6e7f739eba68e33b",
    "chrome/common/chrome_paths_mac.mm":"6829c9cb92c228dd19d44c9f739bf4595720cc2d",
    "chrome/common/chrome_paths_win.cc":"9ab342785936f8e9c8f64ac5088dedcf276058ce",
    "chrome/install_static/chromium_install_modes.h":"a6d969df56d13d94eaa968934f909d6fd23b5f2f",
}

def git_blob_id(data):
    return hashlib.sha1(b"blob "+str(len(data)).encode()+b"\0"+data).hexdigest()

for relative,expected in BLOBS.items():
    destination=ROOT/"browser/upstream-snapshots"/relative
    if destination.is_file() and git_blob_id(destination.read_bytes())==expected:
        continue
    locations=[
        (f"https://raw.githubusercontent.com/chromium/chromium/{REVISION}/{relative}",False),
        (f"https://chromium.googlesource.com/chromium/src/+/{REVISION}/{relative}?format=TEXT",True),
    ]
    errors=[]
    for url,encoded in locations:
        try:
            request=urllib.request.Request(url,headers={"User-Agent":"OmniPlaySourceBootstrap/1"})
            with urllib.request.urlopen(request,timeout=45) as response:
                data=response.read(8*1024*1024+1)
            if encoded:
                data=base64.b64decode(data,validate=True)
            if len(data)>8*1024*1024 or git_blob_id(data)!=expected:
                raise ValueError("Upstream blob identity mismatch")
            destination.parent.mkdir(parents=True,exist_ok=True)
            destination.write_bytes(data)
            print(relative,flush=True)
            break
        except Exception as error:
            errors.append(type(error).__name__)
    else:
        raise SystemExit(f"Could not retrieve {relative}: {','.join(errors)}")
