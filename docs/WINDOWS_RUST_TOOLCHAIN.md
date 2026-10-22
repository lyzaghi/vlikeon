# Local pinned Windows Rust preparation

`tools/prepare_windows_rust.py` assembles official Rust 1.89.0 components for
`x86_64-pc-windows-msvc` into a fresh project-local directory. This matches the
workspace's declared Rust minimum. It downloads rustc, cargo and the target
standard library from the official HTTPS distribution, checks each published
SHA-256 and inventories the selected bin/lib files. It runs no installer or
downloaded tool and changes neither registry nor persistent PATH. Checksums
verify transfer integrity against the distribution; they are not an independent
signature or a supply-chain audit.

Preparation example, from the repository root:

```powershell
python tools/prepare_windows_rust.py --project out/toolchains/rust-1.89.0-windows-msvc-v1
```

The destination must be fresh and within the workspace, with no linked ancestry.
Archives, paths, entry types/counts, per-file sizes and retained expansion have
finite bounds. File collisions reject preparation. Failed/partial directories
are retained without a success receipt; do not execute them or overwrite an
earlier preparation. The script copies only component bin/lib payloads and
never executes archive installer scripts. Original archives retain their notices.

The first successful local preparation contains 63 files / 562,846,115 bytes.
Its receipt records exact official component hashes and every copied file hash,
with tool execution/product build not started and tests not run. Downloaded
archives and tool binaries remain ignored local prerequisites, not public source
or release payloads. This is not a product compile or a qualification result.

MSVC C++ build tools and a Windows SDK remain separate prerequisites. This
machine's existing MSVC installation was located through vswhere; the preparation
script does not install or modify it. A test batch must record actual compiler,
SDK, source and dependency identities when it executes.

Antigravity batch 002 targets the frozen a8b5c4d source and runs only the
native-runtime control worker/channel test filters. It sets RUSTC, CARGO_HOME,
CARGO_TARGET_DIR and tool PATH in its child process only, uses --locked and
retains raw compiler/test output. Explicit batch authorization includes Cargo's
normal locked dependency download and compilation; no unplanned installation,
source/lockfile edit, diagnosis or fix by Antigravity is permitted. Codex reviews
compiler failures, executed test counts and evidence before accepting results.
Native/game/device/release acceptance still requires the original full matrix.

Primary contracts:

- https://rust-lang.github.io/rustup/installation/other.html
- https://rust-lang.github.io/rustup/installation/index.html
- https://learn.microsoft.com/en-us/windows/dev-environment/rust/setup
