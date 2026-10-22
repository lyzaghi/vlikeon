# Desktop browser dashboard source

The shared Game Hub now includes a desktop Browser view. This is authored source,
not a compiled or exercised browser. Android/iOS use their native browsing UI;
the standalone Web/PWA cannot enumerate or manage browser tabs. Those contexts
hide this desktop-only view without calling an unsupported native method.

The component worker implements a separate bounded `browser_*` command namespace
before native messaging. Opening websites, default-provider search, built-in
browser shortcuts and tab navigation therefore do not depend on the local game
library broker. The ordinary Chromium omnibox, navigation controls, privacy UI,
profiles, downloads, permissions and password manager retain their native owners.

## Authority, privacy and bounds

The extension declares `tabs` and `search` permissions. It has no host permissions,
content scripts, page injection, screenshot permission, history/bookmark database
reader or password reader. `tabs` allows sensitive tab metadata; this is a real
permission, not a claim that the component cannot access URLs. The implementation
returns only bounded titles, origin labels and tab states to its own UI. A page
can choose a title containing private information; title truncation is not a
semantic redaction guarantee. Data stays in memory and does not enter the native
broker, diagnostics, recommendations, encrypted game-save sync or analytics.
Native browser history still follows the browser's own navigation/privacy rules.

For each operation, the worker resolves the port sender's browser-issued
`documentId` using `runtime.getContexts`. It requires a normal-profile, top-level
TAB context at this component's exact `index.html`, with no query string. The
window/tab comes from that context, never a renderer-supplied window identity.
Queries, activation and pin changes are restricted to that normal window. No
incognito context or tab is returned or mutated. The extension manifest also
disallows incognito loading. Context disappearance fails closed.

Queries return forty entries per page from a sixty-second snapshot, capped at
2,000 entries with an explicit truncated flag and real total count. Titles are
bounded to 160 Unicode scalar values, origin labels to 256, and controls/bidi
overrides are removed. Text is inserted with `textContent`, isolated direction
and no remote favicon or page preview. Refresh updates the observed states.
Snapshots expire from worker memory and the UI clears tab metadata when leaving
the view. The browser itself owns the initial query array; this API does not
assert a browser-process memory bound independent of the number of open tabs.

Each page admits eight requests in flight, with a global limit of 32 and a bounded
two-minute duplicate-ID set. Its operations serialize so snapshots and guard
mutations do not race within one port. Requests keep the shared 120-second maximum
deadline and recheck liveness before side effects. Browser API calls cannot be
cancelled atomically: an operation that times out may already have opened a tab.
The UI instructs a refresh before retry rather than silently resubmitting it.
No raw browser error, URL, search text or title is logged or placed in errors.

Address entry defaults to HTTPS, accepts explicit HTTP, rejects credentials,
whitespace/control/backslash syntax and non-web executable/internal/file schemes.
Search is a distinct form and uses the browser's selected default provider through
`chrome.search.query`, directed to a tab created in the verified window. Failed
searches leave a visible blank tab; this path never closes user content.
Built-in shortcuts come from a fixed allowlist, not a supplied internal URL.

## Playing-tab memory guard

Before loading a desktop WebAssembly game, the player acquires a page-owned
`autoDiscardable: false` lease for its own Game Hub tab. Acquisition must succeed
before continuing. It stores the lease ID before the request, allowing cleanup
even when the response is lost. Up to four idempotent owner leases share one tab
guard; release is idempotent. The prior boolean is journaled before mutation in
`chrome.storage.session`, using only tab/document IDs and that boolean.

The player renews/verifies its lease on its five-second lifecycle heartbeat and before resuming. Lost protection pauses the player; it does not continuously overwrite a competing actor’s flag. A restarted worker can reconstruct protection from the page-owned lease on renewal, with a recovery/reacquisition gap that remains a required fault case. Normal stop or failed opening releases the player's lease after stopping its
runtime. Port closure attempts cleanup after outstanding serialized operations.
Worker initialization restores journaled values before admitting new operations.
The session journal does not survive browser restart. A failed restoration retains
its marker for a later recovery attempt and surfaces a visible player warning.
The browser keeps control of crash/OOM, explicit user discard/close and other
extensions; the API is not a guarantee against every termination or competing
writer. Restoration is a best-effort return to the observed prior flag, not a
cross-extension compare-and-swap protocol. Autosave and explicit save/export
remain necessary. This source never automatically discards/closes another tab,
changes OS priority, disables security or promises an FPS/memory improvement.

The final consolidated pass must cover real component/new-tab context identity,
worker suspension/restart, profile/incognito boundaries, all fixed routes on the
pinned Chromium build, live tab moves/closure, stale cursors, title/URL hostile
inputs, deadlines, storage/API failures and guard cleanup under active games.
No such API, browser, WASM, UI, privacy or fault test has run.

API contracts: [Tabs](https://developer.chrome.com/docs/extensions/reference/api/tabs),
[Runtime contexts](https://developer.chrome.com/docs/extensions/reference/api/runtime#method-getContexts),
[Default-provider search](https://developer.chrome.com/docs/extensions/reference/api/search).
Built-in settings routes were read from the pinned Chromium `route.ts`; these
source reads do not establish that a compiled product route has been exercised.
