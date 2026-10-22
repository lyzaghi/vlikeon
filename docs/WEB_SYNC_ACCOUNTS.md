# Browser sync account and HTTPS transport source

This extends `WEB_SYNC_CRYPTO.md` with account tickets, explicitly scoped HTTPS
transport and server CORS. It is authored source only. Both browser crypto and
service policies remain disabled, the Web capability is still
`encrypted_sync: false`, cloud opt-in stays false and the shared account UI stays
hidden. No compiler, Cargo resolver, SDK/target installation, server, account,
fetch/preflight, crypto/storage operation, browser, device or test was run.

## Service policy and network boundary

`security/web-sync-services.json` contains a finite inventory of at most eight
canonical HTTPS audiences, with optional plain base paths. Disabled policy must
have an empty inventory. Whitespace, credentials, query/fragment, percent-encoded
or traversal paths, noncanonical ports/hosts and numeric-host shorthands are
rejected. Python production and browser admission use the same intended rules;
their actual cross-language agreement remains part of final verification.

The Web assembler requires the separately approved crypto artifact before adding
enabled sync services. It generates `web/sync-services.json` and merges only
their declared origins into both deployment CSP and service-worker reconstructed
CSP. The current generated service inventory is disabled and empty; no sync
origin was added. The existing service-worker fetch handler does not intercept
POST or cross-origin traffic. Encrypted/account traffic is never an app-shell
cache resource.

`WebSyncHttp` only sends POST to the selected audience's `/v1/account` or
`/v1/sync`. It uses CORS, omitted ambient credentials, rejected redirects,
no-store and no-referrer. The [Fetch Standard](https://fetch.spec.whatwg.org/#http-cors-protocol)
defines the browser's CORS/credentials boundary; ordinary Fetch is used without
an opaque-response workaround or a private-network permission bypass. Request
body length is supplied by the browser; JavaScript does not set Content-Length.

Each request checks the captured durable credential generation and account
identity before sending and after reading its response. Signatures come from
the existing Rust domains/types. Device documents are bounded base64url headers;
root commands contain public keys and signed command bytes. Recovery keys,
device seeds and the data key are never HTTP fields. This transport exposes no
arbitrary URL, caller header, cookie or redirect continuation.

Requests have a 30-second network/body deadline and at most two active fetch
owners per client. Foreground loss, page hide, cancellation, deadline and close
abort those owners. Errors use fixed local messages rather than echoing remote
content or secrets. An accepted server mutation may finish after local abort;
the source does not claim cancellation rolls back remote effects.

JSON responses are bounded at 256 KiB. Typed operations validate device lists,
strictly increasing save-list pages, resumable capabilities, exact blob headers
and upload receipts. Binary download chunks are at most 256 KiB; each must match
its requested length, exact object/header binding and declared chunk SHA-256.
Upload chunks are hashed from their actual bytes and receipts must acknowledge
at least the submitted extent. Only commit or an existing-object receipt can
claim publication. No automatic whole-object or failed-capability downgrade is
provided. These are transport/source checks, not a completed save-reconciliation
or ciphertext-interoperability result.

## Account actions and credential publication

`WebSyncAccount` implements the shared action protocol 1:
`sync_action_start/status/cancel/collect/discard`, with read-only `sync_status`.
One action can be alive per page. The origin's profile-specific Web Lock is
exclusive and nonqueued; it remains held until the actual asynchronous operation
returns, including after cancellation. Missing Web Locks fail closed. Account
and transport source now share the scope-bound lease helper described in
`WEB_SYNC_JOURNAL.md`; complete data-sync jobs still need to own that same scope.

Action status contains only identity, kind, timestamps, state, stop flags and
sanitized errors. Root recovery strings appear only in a bounded completed Create
result. The result has a five-minute observed wall/monotonic expiry, is collected
once, and is cleared on discard/close/expiry. A running action cannot be replaced
or discarded while it still owns work. Cancellation requests stop immediately;
the running state remains until the operation actually exits. The overall action
budget is 110 seconds. Hidden/focus-lost pages cancel active actions and retire
their crypto/HTTP owners; a preserved page can later reopen its delegated session.

| Action | Authored behavior |
| --- | --- |
| Create | Commit a local unregistered device and return its recovery string; no remote registration |
| Register | Require that recovery string plus the service registration code, verify the root acknowledgement and update the same local generation |
| Recover | Derive a new delegated device, verify/enroll through an authenticated one-entry list request, then commit local credentials as registered |
| Renew | Reauthorize the existing delegated device using the matching recovery key; an expired certificate can be renewed |
| Devices | Request and validate the registered account's bounded device list |
| Revoke device | Send a root-signed revocation; acknowledged self-revocation disconnects the local device |
| Delete account | Require a root-signed deletion acknowledgement, then disconnect locally |
| Disconnect | Clear local credentials without requiring the service or a crypto-artifact fetch |

Recovery uses a temporary, operation-bound signer before durable publication.
It compares the original empty/disconnected vault generation before/after each
probe. A wrong/unavailable service does not leave a newly connected local device.
The server can nevertheless have observed enrollment before a cancelled or lost
local acknowledgement; quota, retry and lost-device recovery need real service
evidence. Pending Create is separate from Register so a lost initial recovery
result cannot strand any remotely uploaded data. The UI explains disconnecting
and recreating an unregistered account when its Create result was lost.

The coordinator serializes credential operations, and the vault compares its
current generation before publishing or disconnecting. Late results cannot
restore an older local credential row. Remote acknowledgements lost before
local commit can still require explicit retry/recovery; no distributed atomic
transaction is claimed. Account mutations do not promote a save, synchronize a
ROM/BIOS, opt into data transfer, or report a data-sync job complete.

The shared Settings source can present the product's explicit service choices
and describes browser credential storage separately from the native OS vault.
The recovery display clears on page hide. These UI branches are authored for
the eventual full Web path; the capability flag has not been enabled.

## Server browser-origin admission

`services/sync/src/cors.rs` wraps the existing routes before request body/auth
handlers through the pinned [Axum middleware
API](https://raw.githubusercontent.com/tokio-rs/axum/axum-v0.8.4/axum/src/middleware/from_fn.rs).
`OMNIPLAY_SYNC_WEB_ORIGINS` is an optional JSON array of at most eight exact
canonical HTTPS browser origins. Absence means `[]`, allowing no Origin-bearing
browser request. Duplicate, wildcard, null, HTTP, path and malformed origins are
rejected. Native clients without an Origin header still need the same root/device
signatures and replay/revocation checks.

For the two API routes, preflight accepts only POST and the endpoint's finite
Content-Type/registration or authorization request headers. It rejects duplicate
preflight fields, unexpected headers and a private-network access request. An
accepted origin is echoed exactly with Vary; credential sharing and wildcard
allowance are absent. Responses expose only the chunk/object metadata needed by
the browser and add no-store/nosniff/no-referrer. Root/device authentication,
registered-account checks, nonces, object/transfer quotas and the loopback-only
upstream/TLS-ingress requirement remain in place.

Registration tokens now require 32–256 visible ASCII bytes in both browser and
service startup admission. No actual origin/token, ingress, TLS certificate or
service was configured. A deployed reverse proxy must preserve the exact
audience, route/base-path mapping, CORS/security headers, body limits and the
required known-length encrypted request behavior. Browser/network/platform
access gates are not overridden by this source.

## Source artifacts and remaining scope

The current source-only project is
`out/web-sync/crypto-v2-account-transport`: 162 files / 1,037,011 bytes, with
retained source/recipe/policy/lock inventory. Its name denotes a producer snapshot;
the Rust crypto ABI, account wire and resumable transport remain protocol 1.
Previous source projects are preserved. All six Web/desktop Chromium/Android/iOS
resource/integration assemblies completed without cores. No WASM, native/service
binary or distribution approval was produced.

Durable ciphertext journals, shared leases, quotas and inbox/cursor publication
are authored in `WEB_SYNC_JOURNAL.md`. Authenticated canonical mapping,
conflict/equivalence/tombstone handling, consent epochs, foreground jobs and the
shared account UI integration are now authored in `WEB_SYNC_RECONCILIATION.md`.
Explicit queued cloud repair is now authored in `WEB_SYNC_RECOVERY.md`.
Old upload-stage identity/orphan recovery and complete runtime
qualification remain required. Native/mobile implementations do not establish
those Web paths. The actual artifact/service policies and capability stay off.

The consolidated final pass must cover the actual Rust/WASM/service builds,
native/browser signatures and certificates, exact CORS origins/preflight/error
headers and ingress behavior, Fetch body lengths and HTTP versions, CSP/SW/cache,
invalid metadata/chunks/acks, resource/deadline/cancellation races, staged
recovery publication, expired/revoked/quota-limited devices, root-result loss and
expiry, simultaneous windows and credential generations, BFCache/lifecycle,
service deletion/retry and real target browsers/devices. No such pass was run.
Every other original browser/runtime/platform/streaming/sync/security/licensing/
store/release requirement remains active.
