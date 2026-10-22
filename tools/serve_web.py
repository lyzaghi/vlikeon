"""Explicit loopback-only Web/PWA authoring server. Not a production TLS server."""
import argparse
from http.server import SimpleHTTPRequestHandler,ThreadingHTTPServer
import json
from pathlib import Path
from urllib.parse import unquote,urlsplit
from assemble_web import ROOT,HEADERS

class Handler(SimpleHTTPRequestHandler):
    inventory={}
    root=None
    deployment_headers=HEADERS
    def log_message(self,*args):pass
    def list_directory(self,path):self.send_error(404);return None
    def end_headers(self):
        for key,value in self.deployment_headers.items():self.send_header(key,value)
        self.send_header('Cache-Control','no-store');self.send_header('Service-Worker-Allowed','./');super().end_headers()
    def translate_path(self,path):
        parsed=urlsplit(path)
        try:relative=unquote(parsed.path,errors='strict').lstrip('/')
        except(UnicodeError,ValueError):return str(self.root/'.not-found')
        if relative=='':relative='index.html'
        if parsed.query or parsed.fragment or relative not in self.inventory:return str(self.root/'.not-found')
        target=self.root/relative
        if target.is_symlink() or not target.resolve().is_relative_to(self.root):return str(self.root/'.not-found')
        return str(target)
    def guess_type(self,path):
        try:relative=Path(path).relative_to(self.root).as_posix()
        except ValueError:return'application/octet-stream'
        return self.inventory.get(relative,'application/octet-stream')

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--port',type=int,default=8787);args=parser.parse_args()
    if not 1024<=args.port<=65535:raise ValueError('Choose a bounded nonprivileged local port')
    root=(ROOT/'out/web').resolve(strict=True)
    if root!=ROOT/'out/web':raise ValueError('The web output must not resolve through a link')
    manifest=json.loads((root/'asset-manifest.json').read_text(encoding='utf-8'))
    Handler.deployment_headers=json.loads((root/'deployment-headers.json').read_text(encoding='utf-8'))
    Handler.root=root;Handler.inventory={item['path']:item['mime'] for item in manifest['assets']}
    Handler.inventory.update({'service-worker.js':'text/javascript; charset=utf-8','asset-manifest.json':'application/json'})
    server=ThreadingHTTPServer(('127.0.0.1',args.port),Handler);server.daemon_threads=True
    print(f'OmniPlay authoring resources: http://127.0.0.1:{args.port}/')
    try:server.serve_forever()
    finally:server.server_close()
if __name__=='__main__':main()
