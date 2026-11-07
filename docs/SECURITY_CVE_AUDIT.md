# Security advisory and Chromium freshness audit

The release audit scans the exact workspace Cargo.lock with cargo-audit and an independently selected checkout of the official RustSec advisory database. It queries Windows Stable from Chromium Dash at runtime and compares the result with the Windows pin in browser/upstream.lock.json.

```text
python tools/audit_security_advisories.py --advisory-db <rustsec-checkout> --advisory-revision <official-commit> --cargo-audit <executable> --output <report.json>
```

Use the [official cargo-audit release](https://github.com/rustsec/rustsec/releases/tag/cargo-audit%2Fv0.22.2) and independently verify the commit from the [official advisory repository](https://github.com/RustSec/advisory-db). The report records the lock SHA-256, RustSec commit and tree, number of packages and advisories, and the SHA-256 of the Chromium Dash response. It rejects malformed or zero-package lockfiles, ignored advisories, partial scans, and snapshots older than 14 days. Only a complete scan with no vulnerability findings and a matching Windows Stable version and revision returns exit 0.

| Status | Meaning |
|---|---|
| `CLEAN` | The RustSec vulnerability list is empty and the Windows Stable pin matches. |
| `OUTDATED` | The scan completed and the Windows Chromium pin trails the official Stable release. |
| `VULNERABLE` | RustSec reported affected Rust packages. |
| `UNVERIFIED` | Required evidence is invalid, unavailable or mismatched. |

On 2026-11-08, RustSec commit `309392b18f6b17022f82c701f300433126901068` flagged rustls-webpki 0.103.15 and webpki-roots 1.0.9. Updating them to 0.103.16 and 1.0.10 produced zero vulnerability findings for 327 packages. The Tokio unsoundness warning cleared with 1.53.3. Four unmaintained-package advisories remained as warnings. [RustSec rustls-webpki](https://rustsec.org/advisories/RUSTSEC-2026-0382.html) · [RustSec webpki-roots](https://rustsec.org/advisories/RUSTSEC-2026-0381.html) · [RustSec Tokio](https://rustsec.org/advisories/RUSTSEC-2026-0386.html)

The original 155 pin returned `OUTDATED`. After migrating desktop sources and patches to official Windows Stable `158.0.8127.18` at `6449d9f9a6a906edff724b7b220199b3fde4615c`, the combined source audit returned `CLEAN`. A full browser build and runtime qualification remain open. Android retains an independent source pin pending its own upgrade and device qualification. [Chromium Dash](https://chromiumdash.appspot.com/fetch_releases?channel=Stable&platform=Windows&num=1)
