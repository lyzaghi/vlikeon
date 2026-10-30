# Task 4 — accepted Linux x64 development Chromium build

Batch 004-build-d completed the full chrome target with exit 0 at
2026-10-29T13:33:29Z (204083.92 seconds, one job).
Antigravity independently executed the native checker in this chat's turn 0049:
the raw command completed with exit 0 and passed: true. Codex reviewed the
completed tool output and independently retrieved the remote report on
2026-10-30. Its receipt, batch and selected artifact hashes match the earlier
Codex native acceptance report.

[acceptance-ag-d.json](acceptance-ag-d.json) preserves the returned report
without SSH host details. It binds the unchanged 141 frozen inputs, completed
build journal and receipt, dependency/GN digests, ELF64 x86-64 executable identity
and five selected outputs.
Receipt SHA-256: 65200b9e3daeb5e92171e9b7baef19472ab9bc8e0725ec71dc5435ead3d180ed.
Batch result SHA-256: a6db7fed35c64d364e49f984d06f93f2c0e49040164cec91f60b17b8724253c5.

The recipe records Git/Python identities and pinned depot_tools revision
4be4e4628e093c4ad50210db5dcfa05a88280bda; Chromium revision is
0e66b94549a8bd85f03fbdf189b752d6e7b63c32. Product source tree is
c90b68c0cb3cf9bff4462473a3c8be5969bb54b0, independent of later parallel edits.
The low-memory host had about 4 GB physical RAM and 28 GiB swap, below
upstream's 8 GB physical RAM minimum. Earlier failed/interrupted batch logs
remain preserved.

Acceptance covers the full development build and selected output integrity.
Runtime/native bridge, production cores, release signing/distribution and
clean-clone reproducibility are separate, unaccepted scopes.
See docs/LINUX_CHROMIUM_BUILD.md and preparation-summary.json.

Codex additionally read the completed journal (phase built, running/failed null)
and independently hashed the admitted Git and vpython executables on 2026-10-30:
- Git: 07997209f4513fdee3e1eb151b1c37d56cfe37f9915d331ec37d688c6b9d3ce9.
- vpython: 99597d2f073ce9d1ef940d18ff8b6bb63c0e3860be3c723e5510f7e1bac2d527.
Both match the final receipt and journal identities.
