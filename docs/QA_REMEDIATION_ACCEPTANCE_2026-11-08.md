# QA remediation acceptance on 2026-11-08

Codex accepted the scoped source fixes and regression evidence at `7c21ad707998025d43803e93daaa179e39174cd3`. All five GitHub Actions jobs passed. [Exact CI run](https://github.com/lyzaghi/vlikeon/actions/runs/42427749849)

| Check | Evidence |
|---|---|
| Windows, Ubuntu and macOS | Each clean runner passed repository integrity, Web development assembly, Python qualification, Chromium patch rehearsal, Node regression and the selected Rust package tests. |
| Remote Play | Browser RemoteSession decoded video over real WebRTC, performed an ICE restart, revoked input and closed on capture loss. The job also passed relay-only and TURN fallback fixtures. The fallback fixture withholds direct candidates. Signaling in this test is in memory. |
| Security | The official cargo-audit release was checksum verified. A fresh official RustSec snapshot scanned the patched lockfile. The Windows Chromium pin matched official Stable and the combined source audit returned CLEAN. |
| Chromium source migration | All 36 candidate input files matched official Git blobs for 158.0.8127.18. Both migrated patches applied against that candidate, and nine source security invariants passed. |
| Local clean checkout | At `cd1681b`, Windows passed 47 Rust tests and 11 doctests, 33 Node tests and 11 Chromium tests, plus installer, security and platform qualification suites. Broker tests ran serially because they share process-wide worker state. The source tree remained clean. |

The installer guard rejects malformed process records and lookup failures before writing or deleting files. It distinguishes installed product Chromium from an unrelated Chrome executable by its image path. The staged standalone uninstaller includes the shared guard module.

The CVE audit uses cargo-audit and official data. Missing or malformed input, zero inspected packages, stale data and incomplete scans fail the gate. A real lockfile fixture containing the three previously missed affected versions produced all three expected RustSec findings. The patched production lockfile has zero vulnerability findings. Unmaintained-package warnings remain visible in the audit report.

Native SRTP now uses AES-128 counter mode, the RFC 3711 key derivation and HMAC-SHA1-80. Its key derivation matches the published vectors. UDP receiver tests reject replay and retired-session packets. The DTLS key adapter accepts negotiated exporter material and verifies the expected certificate fingerprint. A native DTLS handshake integration remains a separate transport requirement.

Android retains its independently pinned source revision. Its pin, snapshot inventory and patch queue are checked for consistency. Desktop source migration leaves Android upgrade and device qualification open.

## Remaining release work

1. Compile the full Chromium 158 browser and repeat browser runtime and release artifact qualification.
2. Exercise release-package installation, upgrade, crash, repair and rollback on clean target OS machines.
3. Qualify production signaling, TURN deployment, WAN impairment and physical capture, audio and input hardware.
4. Complete the existing mobile and remaining 109-item ledger gates, including Android security migration and external release requirements.

This receipt accepts the scoped remediation. Production release approval remains open.
