# Task 6 — actual browser runtime qualification in progress

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
remain required.

Runtime stderr also records a delay-load exception `0xc06d007e` in a subprocess,
with a user32 delay-load stack, and an AMD video-processor warning. These require
diagnosis; launching the main process does not qualify the subprocess/runtime.
No sandbox disable, extension sideload or mocked native-host success is used.

Remaining acceptance includes normal/private profile separation and persistence,
real tab operations, permission admission/rejection, native broker registration
and actual connection/failure handling. A manifest incognito policy or absent
ordinary-page native API alone cannot prove these broader requirements. There
is currently no task-6 acceptance or release approval.
