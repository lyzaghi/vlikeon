"""Install only the fixed-origin native host manifest for an OmniPlay profile."""
import argparse
import json
import os
from pathlib import Path
import sys

ROOT=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument("--executable",type=Path,required=True)
parser.add_argument("--browser-data-directory",type=Path)
args=parser.parse_args()
executable=args.executable.resolve(strict=True)
if not executable.is_file() or executable.name not in {"omniplay-broker","omniplay-launcher"} or not os.access(executable,os.X_OK):
    raise SystemExit("Select the executable OmniPlay broker or stable launcher")
if args.browser_data_directory:
    profile=args.browser_data_directory.expanduser().resolve()
elif sys.platform=="darwin":
    profile=Path.home()/"Library/Application Support/OmniPlay"
elif sys.platform.startswith("linux"):
    profile=Path(os.environ.get("XDG_CONFIG_HOME",str(Path.home()/".config")))/"omniplay"
else:
    raise SystemExit("This installer supports macOS and Linux")
identity=json.loads((ROOT/"security/component_identity.json").read_text(encoding="utf-8"))
directory=profile/"NativeMessagingHosts"
directory.mkdir(parents=True,exist_ok=True)
manifest={"name":"com.omniplay.broker","description":"OmniPlay local game library and progress service","path":str(executable),"type":"stdio","allowed_origins":[identity["origin"]]}
destination=directory/"com.omniplay.broker.json"
temporary=directory/".com.omniplay.broker.pending"
with temporary.open("x",encoding="utf-8") as stream:
    json.dump(manifest,stream,indent=2)
    stream.write("\n")
    stream.flush()
    os.fsync(stream.fileno())
os.chmod(temporary,0o600)
os.replace(temporary,destination)
print("OmniPlay native host manifest installed for the selected browser profile.")
