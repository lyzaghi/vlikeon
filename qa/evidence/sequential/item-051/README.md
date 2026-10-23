# Item 51 acceptance

`acceptance.json` maps the archive-interchange requirement to 25 groups executed
by Antigravity against real Windows native and Chrome/IndexedDB product APIs.
Four existing native backup/recovery regression fixtures also passed.

`051g` is the final complete execution. Earlier failed results remain under
`051a`, `051b`, `051c`, `051e`, `051f`; `051d` was an earlier partial-matrix pass.
They are not counted as additional coverage. `source-051g.json` identifies
actual frozen inputs; `runtime_inputs` records the current relevant source
files checked against that manifest. Documentation and acceptance were updated
after execution. The native and Web fixes are described in
`docs/SAVE_ARCHIVE_INTERCHANGE.md`.

Synthetic archive samples of at most 64 KiB accompany the final result. Hashes
and byte lengths of all generated archive fixtures are in acceptance. Actual
128 MiB and other large boundary artifacts remain locally under
`E:\V10\work\vlikeon-items-50-109-01a16780\runs\051g\exchange`; the repository
fixture reproduces them. No user game/profile data is included.

The runner uses machine-specific absolute tool paths, an immutable copied
source tree and fresh outputs. Reproduction requires equivalent toolchains and
new paths. Antigravity CLI completion alone is not acceptance; Codex reviewed
actual command exits, the exercised implementation, source identity and
artifact results. See the acceptance limits for separate item 52 fault/schema,
OS/device, picker/provider and PWA/browser packaging gates.
