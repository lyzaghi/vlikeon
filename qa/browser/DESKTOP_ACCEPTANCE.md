# Item 1 final acceptance evidence

This is the authored execution matrix for the later single consolidated pass.
Nothing in this matrix has been run. Record target, exact built artifact,
commands, observed results and retained evidence for every case. A mocked unit
case supplements the matching live Chromium case and cannot replace it.

| Case | Required observed behavior |
| --- | --- |
| Windows x64 / Linux x64 / macOS arm64 build | Exact frozen overlay builds with its native SDK and pinned tools; entry point and dependency inventories match the recorded build receipt |
| Rebuild comparison | Independent build inputs and outputs are compared; differences are explained, never labelled reproducible from a single successful build |
| First run and new tab | OmniPlay product identity, normal browsing controls and bundled Game Hub load; no missing GRIT resources or worker imports |
| Two normal profiles | Library/profile IDs, extension data, tab commands and native requests remain separate across profiles and browser restart |
| Private window | Game Hub is not loaded and no private context acquires library, browser-tab or native capability |
| Foreign/subframe/replaced document | Native/browser requests fail; changing request tab/window/profile metadata cannot widen authority |
| Two Game Hub tabs in one profile | Session/preparation/input/media/file-grant/import/transfer IDs acquired by one document cannot be used by the other; session listings include only document-owned sessions |
| Native service unavailable | Ordinary browsing/dashboard remain usable; library failure is actionable and reconnection does not replay mutating requests |
| Profile identity corrupt/storage denied | No replacement library is silently created; no request reaches a broker with a guessed/new identity |
| Worker restart | Session-only discard guards recover; page-owned leases renew or fail visibly; profile identity remains stable |
| Native port disconnect/reconnect | Old callbacks cannot clear a new port, leak replies to another document or resurrect prior capability ownership |
| Late response/page close | Session/preparation/input/export/media results are retired through their correct bounded owner paths |
| Deadline and overload | Expired requests are not forwarded after authentication; bounded overload stays recoverable without duplicate side effects |
| Sustained native traffic | More than 1,024 file chunks/input/status requests can progress in one window; rotating replay retention never evicts in-flight/late-cleanup IDs or turns the cache capacity into a throughput quota |
| Browser dashboard | Fixed shortcuts, default-provider search, current-window paging/activation/pinning and hostile title/URL inputs obey the documented authority boundary |
| Game tab protection | Acquire/renew/release, external flag changes, tab move/close and failed journal restoration produce correct behavior without affecting unrelated tabs |
| CSP and extension scope | Only packaged scripts run; arbitrary origins have no extension/native message route; iframe/query/foreign extension paths are denied |
| Native host discovery | Broker is discovered only through the product-specific registration; absent/malformed registrations leave browsing available |
| Upgrade rehearsal | Candidate upstream revision preserves the complete patch/resource contract and profile data; an old snapshot mismatch refuses destructive overwrite |

The source unit cases in `component-context.test.mjs` and
`component-ownership.test.mjs` cover document admission and transient handle
ownership. They do not prove UI, native messaging, sandbox, SDK, rebuild or
runtime behavior. Sign-off for the full item requires the real platform matrix.
