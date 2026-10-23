# Installed Windows video worker admission — item 56

Source only. No worker/package was installed, admission executed, native capture/
launch performed or build/test run. Policy remains disabled. No current service
command invokes the new private handoff.

`stream_video_installation::admit` uses the updater's active installed release
and embedded channel. This broker's current executable must match that release's
verified broker entry. Only `helpers/omniplay-video-worker.exe` is admitted: its
inventory entry must be executable, nonempty, at most 128 MiB, with exact hash
and length. There is no renderer path/hash, build-output fallback or download.

Non-delete-sharing directory handles retain the version root, its ancestors and
helpers directory. Backup semantics/OPEN_REPARSE_POINT directory opens reject
reparse/non-directory objects. The ordinary non-reparse worker file uses read
sharing only, excluding write/delete. The SAME retained file handle is hashed
and ordinary updater path resolution rechecked. Failure never weakens sharing
or changes ACLs. Installed-root ACL/provenance/updater trust requirements and
real filesystem race/rename/activation qualification remain required.

Directory guards travel into VideoWorker Shared with the file guard and survive
private I/O/watchdog retirement. Unknown process termination retains all guards,
original job/process and singleton permit in the bounded tombstone. A dropped
broker wrapper cannot release them while I/O still holds Shared. Raw/ad-hoc
entry points still require their own caller admission; arbitrary File/path
values do not become trusted because this installed route exists.

`StreamInputs::spawn_owned_video` privately matches a prepared native capture's
capture/game/pair/session/single generation, verifies the retained original
process PID/creation identity and bounds lifetime by capture expiry and host
deadline. It consumes one launch attempt before installed admission, preventing
replay after partial setup. Remaining lifetime is recomputed after slow hashing.
The typed inherited-handle v2 launcher is used; fresh capture/consent is required
after failure. No public service command calls this handoff yet.

The HWND must originate from an admitted native registration; the helper still
rechecks it and presents local confirmation. The returned authority lease must
enter the independent observer lane before Configure or its initial 300 ms lease
expires. Signed pairing/capture preparation alone does not prove current native
document/pair authority. Capture release/replacement must reach the future real
observer; this private launch function does not wire that lifecycle signal.

Remaining: actual built/signed helper packaging/inventory, trusted native HWND/
document/capture/pair-revocation sources, service lifecycle/Stop UI, receiver
codec/VUI admission, transport/publication fences and full original platform/
device/fault/latency qualification. No feature is enabled to hide missing paths.
