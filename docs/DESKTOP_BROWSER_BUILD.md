# Desktop Chromium integration and build source

Item 1 covers the desktop Chromium product: the revision-bound patch queue,
bundled Game Hub, normal/private profile separation, browser-issued document
authority, product-specific native-host discovery, and a repeatable build
procedure for Windows x64, Linux x64 and macOS arm64. The current implementation
is source only. No Chromium configuration, dependency hooks, compilation,
browser execution or tests have run.

## Integration and authority

`tools/generate_browser_patch.py` authors the small desktop queue against
`browser/upstream.lock.json`. It retains upstream browser UI and security
defaults, bundles inventoried local component resources and isolates product
profile/native-host locations. Game Hub is not loaded in off-the-record
profiles. Its manifest rejects incognito, uses a fixed public component key and
contains no content scripts or external web-origin connections.
Source authoring derives the extension ID from the public-key fingerprint and
requires the bundled manifest, component allowlist, declared origin and native
broker origin to agree. A new upstream lock cannot silently reuse old snapshots;
it requires explicit snapshot and anchor migration.

`component-context.js` is shared by browser commands and native forwarding.
Sender admission requires this component's exact top-level `index.html`, a
browser-issued document ID and a normal profile. Each native request resolves
that document through `runtime.getContexts` before profile access and again
after asynchronous profile initialization. Closed, replaced, subframe,
incognito and unrelated component documents fail admission. Up to eight
document authentication operations per page and 32 overall may be in flight.
Browser tab/window authority continues to come from the live context.

The worker creates the native profile UUID only on the first authenticated
request. An existing invalid or nil identity fails instead of silently creating
a replacement library. Local extension storage is limited to trusted extension
contexts. The worker overwrites request profile/protocol metadata with its own
values. The broker independently fixes the first profile for the entire native
channel and refuses a switch. The native replay cache retains the most recent
1,024 per-document request IDs for at most two minutes, while preserving active
and late-cleanup correlations. Completed entries rotate instead of imposing a
256-request/two-minute throughput limit on file chunks, input or status polls.
The broker's independent exact-byte replay policy still applies. Old native-port
disconnect events cannot clear a later connection.
Late session/preparation creation results are retained for bounded cleanup after
timeouts and document closure. Results arriving outside that retention window
still depend on the broker's independent session/job lifetime controls.

Several normal Game Hub tabs may share one profile broker, but transient
authority is checked per document. A separate ownership boundary verifies native
session/preparation IDs, web-session heartbeats, remote-input grant/session
pairs, media jobs/sets, selected import/export/folder grants, direction-specific
transfers and import jobs before forwarding. Session listings are filtered to
the requesting document's sessions. A renderer cannot shut down the shared
broker. Document closure retires that document's retained grants/transfers/jobs;
late successful creation is retired instead of handed to a replacement page.
Cleanup is tied to the same native-port generation and cannot target a later
reconnected broker. Pending response records retain only handle identities,
never account secrets or transfer payloads. The component boundary bounds open
documents and handle collections; native runtime/file/job limits remain
independent requirements.

`stage_browser_integration.py` retires obsolete generated component assets
through the confined source-asset copier, preventing removed privileged modules
from surviving an incremental staging operation. It preserves source files with
unrelated local changes.

## Frozen source preparation

First author the current overlay with `generate_browser_patch.py`. Then use
`prepare_desktop_browser.py --project <fresh-out-directory> --target <target>`.
The `--development` flag must match the overlay's mode. Release preparation
requires the separately approved core catalog; an empty development catalog
cannot become a release by selecting another GN file.

Preparation freezes every exact modified upstream file, authored replacement,
component resource, patch, product identity, target GN file and the build recipe.
Its receipt records file sizes and SHA-256 identities. The only work performed
by preparation is reading/copying source and writing inventory metadata.

## Explicit later build

`build_desktop_browser.py` requires `--final-verification`, the prepared project,
an explicit Git executable, Python executable, clean pinned depot_tools checkout
and its full commit, plus a finite `--jobs` count. It accepts only a matching
native desktop build host. The operator must provision that host's upstream
compiler, SDK and OS build prerequisites; the recipe does not install or change
those system components.

The recipe creates a separate owned gclient workspace within the prepared
project. It synchronizes the fixed Chromium repository at the exact revision,
runs upstream dependency hooks, preflights all modified destinations and then
copies only the frozen overlay. A persisted journal retains completed phases;
an interrupted overlay accepts only the exact original or new file bytes.
Unrelated edits fail. The existing sparse authoring checkout is never reused or
reset by this procedure. A per-project exclusive lock prevents concurrent runs;
after a crash, an operator must establish that no prior build is still active
before removing its stale lock.

GN and dependency toolchains come from the pinned upstream hooks. The recipe
uses the selected depot_tools `autoninja.py` and the ordinary `chrome` target.
Remote execution and depot_tools auto-update are disabled. Release configs
currently select PGO phase zero; measured PGO/profile qualification belongs to
the later performance milestone. Upstream sandbox, site isolation and
certificate behavior keep their defaults. No custom proprietary-codec/DRM or
API-key entitlement is inferred from a successful build.

A build receipt is written only after compilation returns successfully and the
target entry point exists. It binds the source receipt, depot_tools commit,
explicit tools, GN, actual dependency revision report and browser entry-point
bytes. macOS selection reads the built bundle's product ID/executable identity.
The build entry point never starts or installs the browser, signs a release or
claims reproducibility from one output. GUI behavior, native-host integration,
two-build comparison, SDK/dependency/license admission and item-16 verification
remain separate evidence requirements.

## Completion gate for item 1

The source-only `prepare_chromium_upgrade.py` provides a separate upstream
maintenance review. It takes an explicit newer candidate commit already present
in a Chromium checkout, bounds and binds the candidate Git blobs and VERSION,
and freezes the current original, OmniPlay replacement and candidate original
for every integration source. It authors the upstream diff and changed-source
inventory without fetching, applying/checking a patch, changing the active pin
or touching the authoring checkout. Missing local objects fail with lazy fetch
disabled. Changed code still requires anchor/policy migration; unchanged code
still requires the candidate desktop build and acceptance matrix. Candidate
selection does not grant release approval or imply security-update freshness.

Source completion requires the three target recipes, live-document/profile
boundary, current complete overlay and an upgradeable minimal patch queue.
Verified completion additionally requires actual desktop builds, first-run and
new-tab behavior, profile/private-window isolation, native-host failure handling,
permissions/CSP, worker restart/timeout/cleanup cases and a rebuild comparison.
These runtime checks are deferred to the user's final consolidated pass.
The authored case matrix is `qa/browser/DESKTOP_ACCEPTANCE.md`, with narrow
source-level document admission cases in `component-context.test.mjs`. Neither
has been executed.

Upstream build contracts consulted for the recipe:

- https://chromium.googlesource.com/chromium/src/+/HEAD/docs/windows_build_instructions.md
- https://chromium.googlesource.com/chromium/src/+/HEAD/docs/linux/build_instructions.md
- https://chromium.googlesource.com/chromium/src/+/HEAD/docs/mac_build_instructions.md

