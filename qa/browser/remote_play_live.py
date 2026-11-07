"""Run the production browser RemoteSession against two real WebRTC peers on loopback."""
import argparse
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
from queue import Empty, Queue
import re
import subprocess
import tempfile
from threading import Thread

ROOT = Path(__file__).resolve().parents[2]
PAGE = ROOT / "qa/browser/remote-play-live.html"
STREAMING = ROOT / "engines/streaming"


def run(browser: Path, output: Path, turn_url: str | None) -> dict:
    result_queue = Queue(maxsize=1)
    turn = None
    if turn_url:
        turn = {"urls": [turn_url], "username": os.environ["VLIKEON_QA_TURN_USER"],
                "credential": os.environ["VLIKEON_QA_TURN_PASSWORD"]}

    class Fixture(BaseHTTPRequestHandler):
        def send(self, body: bytes, content_type: str):
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            if self.path == "/":
                self.send(PAGE.read_bytes(), "text/html; charset=utf-8")
            elif self.path == "/configuration":
                self.send(json.dumps({"turn": turn}).encode(), "application/json")
            elif re.fullmatch(r"/engines/streaming/[a-z0-9-]+\.js", self.path):
                path = STREAMING / self.path.rsplit("/", 1)[1]
                if not path.is_file():
                    self.send_error(404)
                    return
                self.send(path.read_bytes(), "text/javascript; charset=utf-8")
            else:
                self.send_error(404)

        def do_POST(self):
            length = int(self.headers.get("Content-Length", 0))
            if self.path != "/result" or not 0 < length <= 8192 or result_queue.full():
                self.send_error(400)
                return
            result_queue.put(json.loads(self.rfile.read(length)))
            self.send(b"OK", "text/plain")

        def log_message(self, *args):
            pass

    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(dir=output.parent) as profile, ThreadingHTTPServer(("127.0.0.1", 0), Fixture) as server:
        server_thread = Thread(target=server.serve_forever, daemon=True)
        server_thread.start()
        try:
            with output.with_suffix(".browser.log").open("wb") as browser_log:
                command = [
                    str(browser), "--headless=new", "--no-first-run", "--no-default-browser-check",
                    "--disable-background-networking", "--autoplay-policy=no-user-gesture-required",
                    f"--user-data-dir={profile}", f"http://127.0.0.1:{server.server_port}/",
                ]
                with subprocess.Popen(command, stdout=browser_log, stderr=subprocess.STDOUT) as proc:
                    try:
                        report = result_queue.get(timeout=90)
                    except Empty:
                        report = {"passed": False, "error": "Timed out waiting for browser report",
                                  "browser_exit_code": proc.poll()}
                    finally:
                        if proc.poll() is None:
                            proc.terminate()
                            try:
                                proc.wait(timeout=10)
                            except subprocess.TimeoutExpired:
                                proc.kill()
                                proc.wait()
        finally:
            server.shutdown()
            server_thread.join()
    if turn_url and report.get("passed") and not report.get("relay"):
        report = {"passed": False, "error": "Required relay evidence is missing"}
    output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--browser", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--turn-url", help="Optional local TURN fixture, with credentials supplied via environment")
    args = parser.parse_args()
    report = run(args.browser.absolute(), args.output.absolute(), args.turn_url)
    print(json.dumps(report, indent=2))
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
