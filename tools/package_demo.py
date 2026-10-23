"""Assemble an isolated playable demo; no production core/catalog activation."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
from build_demo_rom import build

ROOT=Path(__file__).resolve().parents[1]
def sha(data): return hashlib.sha256(data).hexdigest()
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--core',type=Path,required=True)
    parser.add_argument('--executable',type=Path,required=True)
    parser.add_argument('--webview-sdk',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args(); output=args.output.absolute()
    if output.exists() or not output.resolve().is_relative_to(ROOT/'out'): raise ValueError('Choose a fresh demo folder inside workspace out')
    core=args.core.resolve(strict=True); metadata=json.loads((core/'manifest.json').read_text())
    wasm=(core/'sameboy.wasm').read_bytes()
    if metadata.get('source_revision')!='26a56bd6a122a6318a456abd80558408742d5adf' or metadata.get('wasm_sha256')!=sha(wasm) or metadata.get('wasm_bytes')!=len(wasm): raise ValueError('Core provenance mismatch')
    if not 0<len(wasm)<=16*1024*1024: raise ValueError('Demo core budget')
    inputs={}
    def copy(source,target):
        source=source.absolute()
        if source.is_symlink() or not source.is_file() or source.stat().st_size>16*1024*1024: raise ValueError('Expected ordinary bounded demo input')
        data=source.read_bytes(); destination=output/target; destination.parent.mkdir(parents=True,exist_ok=True); destination.write_bytes(data)
        inputs[target]={'bytes':len(data),'sha256':sha(data)}
    copy(args.executable,'VlikeonDemo.exe')
    for name in ['index.html','demo.js','demo.css']:copy(ROOT/'apps/demo'/name,'assets/'+name)
    copy(ROOT/'branding/vlikeon/svg/vlikeon-open-view-neon.svg','assets/logo.svg')
    for source in sorted((ROOT/'engines/web').glob('*.js')):copy(source,'assets/runtime/'+source.name)
    for name in ['sameboy.js','sameboy.wasm','manifest.json','LICENSE.sameboy.txt','LICENSE.emscripten.txt']:copy(core/name,'assets/cores/sameboy/'+name)
    for source,target in [(ROOT/'LICENSE','LICENSE.txt'),(args.webview_sdk/'LICENSE.txt','LICENSE.WebView2.txt')]:copy(source,target)
    rom=build(); destination=output/'assets/games/vlike-grid.gb'; destination.parent.mkdir(parents=True);destination.write_bytes(rom)
    manifest={'schema_version':1,'core':{'id':'sameboy-web','bytes':len(wasm),'sha256':sha(wasm)},'game':{'id':'vlike-grid','bytes':len(rom),'sha256':sha(rom),'license':'Apache-2.0'}}
    (output/'assets/demo-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    about='''Vlikeon Game Hub Demo 0.1 — Windows x64

This is a playable technical demo, not the complete Vlikeon Chromium browser.
It runs the original Vlike Grid Game Boy program through the existing isolated
WebAssembly runtime and a pinned SameBoy build. No commercial game or
manufacturer boot ROM is included. SameBoy supplies its original open-source
boot programs. Original Vlike code/game: Apache-2.0.

SameBoy 1.0.4, revision26a56bd6a122a6318a456abd80558408742d5adf: MIT/Expat.
See assets/cores/sameboy/LICENSE.sameboy.txt and LICENSE.emscripten.txt.
WebView2 SDK1.0.3537.50 loader: see LICENSE.WebView2.txt beside the executable.
The installed Microsoft WebView2 Runtime remains separately managed by Microsoft.
Source: https://github.com/lyzaghi/vlikeon

Use arrows/WASD to collect orbs. Space pauses/resumes. Checkpoints are local
IndexedDB records inside this demo folder's user-data directory. Keep that
directory when updating. Closing the app attempts a final checkpoint; failure
keeps the app open or asks before discarding the current session.
No accounts, cloud saves, commercial game import or arbitrary browsing in this demo.
'''
    (output/'assets/ABOUT.txt').write_text(about,encoding='utf-8')
    (output/'HUONG-DAN.txt').write_text('''VLIKEON DEMO — WINDOWS x64

1. Giải nén toàn bộ vào một thư mục bạn có quyền ghi.
2. Mở VlikeonDemo.exe. Máy cần Microsoft Edge WebView2 Runtime.
3. Bấm Chơi ngay. Di chuyển bằng mũi tên hoặc WASD để nhặt quả cầu.
4. Space: tạm dừng/tiếp tục. Lưu điểm dừng và Khôi phục để thử lưu tiến trình.
5. Đóng rồi mở lại và chơi, sau đó chọn Khôi phục để tiếp tục.

Điểm lưu nằm trong user-data ngay cạnh chương trình. Đừng xóa thư mục này
nếu muốn giữ tiến trình. Đây là bản demo kỹ thuật, chưa phải trình duyệt hoàn chỉnh.
Chương trình chưa ký số. Không yêu cầu quyền quản trị. Không chứa game thương mại.
''',encoding='utf-8')
    (output/'build-receipt.json').write_text(json.dumps({'schema_version':1,'demo_only':True,'production_catalog_changed':False,'inputs':inputs,'game':manifest['game']},indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'output':str(output),'core_sha256':sha(wasm),'game_sha256':sha(rom),'files':len(list(output.rglob('*')))}))
if __name__=='__main__':main()
