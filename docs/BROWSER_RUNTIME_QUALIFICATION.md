# Task 6 — actual browser runtime qualification in progress

On the owner's explicit quota recheck, normal approval and Antigravity execution
worked again. No usage percentage was available. Codex compiled a further local
candidate that initializes the network utility's MTA COM infrastructure before
LowerToken, retaining all sandbox policies and the preceding cosmetic-call
guard. Antigravity's strict fresh-profile test failed: OLE32 initialization
still crashed, now on the main thread before LowerToken. This rejects a simple
delayed-lockdown timing explanation as sufficient and does not qualify the
candidate. It was not promoted to the product patch queue. Further loader
diagnostics are required; task 6 remains open.

Latest local candidate evidence: a compiled startup-recovery hook did not fix
fresh-profile startup and was reverted. A subsequent network-only candidate
skips the cosmetic USER32 startup cursor calls without changing sandbox policy.
Its fresh-profile Game Hub rendered, but Network Service still crashed loading
OLE32.dll for CoInitializeEx on a shared COM worker. Offline dump analysis
confirmed error 1114 again; the underlying initialization cause is unproven.
Antigravity's attribution of that error to lockdown policy is an inference,
not established by the dump, and is not accepted as a root-cause finding.

Neither candidate is promoted to the product patch queue. The second test's
ordinary target remained about:blank, so its API-absence check is insufficient.
A stricter local harness now requires the exact controlled data-page title,
body, URL and completed load, and rejects fatal/network-service log entries.
That revised harness is authored but has not executed. Task 6 remains open.

The owner authorized starting task 6 while the full Linux and Mac build attempts
continue independently. This task covers actual Game Hub, tabs, normal/private
profiles, permission boundaries and the native bridge. It remains unchecked.

Antigravity executes tests; Codex reviews evidence and owns fixes. Initial tests
use the accepted frozen Windows development executable recorded in
`WINDOWS_CHROMIUM_BUILD.md`, with a fresh owned QA profile for each batch. They
do not validate later parallel-fork source or use the owner's browser profiles.

The first runtime batch launched the actual executable and obtained its CDP
version, but failed the Game Hub check after directly creating a chrome://newtab
target. The exact upstream `NewTabUIConfig` rejects that real URL for normal
profiles unless the normal new-tab rewrite has already occurred. A second batch
used the startup new-tab path, observed the bundled component's active service
worker, but failed by selecting page targets only by chrome-extension URL.
Both batches/logs are retained; neither is reported as passing or as proof of
a product defect. A third check inspects the actual DOM/runtime of the startup
tab even when CDP retains a virtual new-tab URL. Its original Game Hub assertions
remain required. That third batch subsequently also failed: the selected actual
startup page DOM remained `about:blank` without an extension runtime ID across
the bounded observation. Its component background worker was active. The
failure now reproduces beyond the initial target-URL selection mistake, but
the cause (startup override, component registration or runtime subprocess
failure) is not established. Codex must diagnose before changing product code.

Runtime stderr also records a delay-load exception `0xc06d007e` in a subprocess,
with a user32 delay-load stack, a network-service crash/restart, and an AMD
video-processor warning. These require
diagnosis; launching the main process does not qualify the subprocess/runtime.
No sandbox disable, extension sideload or mocked native-host success is used.

Remaining acceptance includes normal/private profile separation and persistence,
real tab operations, permission admission/rejection, native broker registration
and actual connection/failure handling. A manifest incognito policy or absent
ordinary-page native API alone cannot prove these broader requirements. There
is currently no task-6 acceptance or release approval.

## Subsequent diagnosis and native-host provisioning

Antigravity analyzed the existing owned QA crash dump using Microsoft's local
debugger and local build symbols. The exact failing process is the sandboxed
`network.mojom.NetworkService` utility. `DelayLoadInfo` identifies USER32.dll,
PostThreadMessageW and Windows error 1114 (DLL initialization failure), reached
from CommonSubprocessInit. This establishes the crash site and loader error;
it does not establish which policy or initialization condition triggered it.
USER32 appears in the dump's module map, so a missing DLL file is not proved.
Raw dumps and process memory stay in private QA storage.

An instrumented navigation diagnostic subsequently loaded chrome://version,
the actual bundled Game Hub document and chrome://newtab rewritten to that
document. The initial controlled data navigation timed out and was aborted
when the next navigation began. The page's scripts attempted native messaging,
which revealed a second concrete issue: the broker manifest was absent.

Codex built the real development broker from the existing task-3 frozen source
export with `cargo build --frozen`, in a new QA target directory. The build
completed successfully. Its executable SHA-256 is
`6cdc00319d59f76c7a4b8a0e233d2e7c17448cb458b9c8045f24262a3bd0493d`.
After confirming no prior product host registration existed, Codex used the
frozen development installer to register only the exact component origin under
the product-specific current-user native-host key. This is development setup,
not an Authenticode-signed distribution. Actual browser/native tests redirect
LOCALAPPDATA to an owned QA directory; personal data is not used.

Extending first-profile startup observation to 60 seconds still left a blank
document. Enabling the CDP Runtime domain did not repair that startup. A restart
of the same initialized QA profile then passed the initial Game Hub DOM and
ordinary-page privilege smoke, with no delay-load or network-service crash in
that run. The old debug endpoint file initially confused the restart harness;
Codex corrected it to wait for a newly written endpoint before the valid test.
This establishes a first-profile versus initialized-profile difference, not a
completed causal proof or permanent startup fix. Native protocol, true private
profile isolation and permission cases remain part of the open task.

A further Antigravity run made an actual `PlatformBridge.request('hello')`
from the loaded component document to the registered real broker. It returned
application OmniPlay, protocol 1.0, version 0.1.0 and platform windows, with the
test exiting 0. This verifies that specific native protocol round trip on the
initialized QA profile. Capabilities in hello are advertised metadata, not
independent qualification of every module. No personal data or account was
used. Identifiers from the private result are excluded from the public summary
at `qa/evidence/sequential/item-006/diagnosis-summary.json`.
