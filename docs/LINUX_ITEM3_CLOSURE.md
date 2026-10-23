# Item 3: Linux native runtime lifecycle and containment

On 2026-10-23 the owner deferred the unfinished Windows item 2 and selected
items 3 through 17. Item 17 is the overall review and closure of outstanding
work. This document tracks Linux implementation separately from execution.

## Current source work

- Correct invalid Rust declarations in the host entry, cgroup discovery and
  socket-activation admission. Propagate cgroup basename query errors before
  comparing the exact service/manager names.
- Keep a malformed, disconnected or refused client from terminating the shared
  native host. Listener and resource/quiescence admission errors remain fatal;
  no alternative delegation or weaker grant is introduced.
- Align ancillary buffers and adopt every received descriptor before rejecting
  extra descriptors, malformed replies or early EOF. Support fragmented stream
  replies without sending the same descriptor twice. Received cgroup authority
  still requires O_PATH, exact owner/mode and cgroup-v2 validation.
- Bound request/reply reading by a single five-second deadline and preserve the
  listener deadline across signal interruption.
- Use a nonblocking connect and a finite completion deadline before restoring
  blocking I/O; refuse a full Unix listener backlog instead of blocking before
  the read/write deadlines exist.
- Add actual Linux userland socket tests for fragmentation, CLOEXEC, descriptor
  closure, refused/corrupt replies, ordinary-file refusal and a bad client
  followed by a valid client on the same listener. These require target execution;
  they do not establish cgroup, systemd or compositor compatibility.

## Remaining closure and acceptance

1. Build/check the exact Linux broker, sandbox, runtime, bootstrap and SDL
   presentation artifacts. Syntax-only parsing on Windows is explicitly
   insufficient for target types, libc ABI, linking or runtime behavior.
2. Complete migration/repair policy around active delegated trees without
   changing live resource ceilings or using an unowned path/PID as authority.
3. Complete interrupted creation/recovery paths and retained legacy outcomes;
   preserve unknown originals, exact lease/identity proof and dying charges.
4. Complete compositor/user-gesture activation and reliable focus/pause/save
   behavior with production X11/Wayland, input and audio devices.
5. Complete actual physical writable-resource accounting for the profile,
   image/cache/work/helper/core and inherited desktop resources. Copy-debt
   reservations are not measured physical storage limits.
6. Execute legitimate systemd delegation/socket activation, lifecycle faults,
   process-tree/resource retirement, filesystem crash recovery and all original
   core/device/SDK/license/package acceptance on supported Linux targets.

The current Windows machine reports WSL is not installed. No Linux environment
has been installed or claimed available. Source changes and authored tests
remain unverified on Linux until exact target evidence is recorded.

The manual GitHub Ubuntu workflow now provides actual target compiler evidence.
Batch036 was rejected before execution due to workflow expression context.
Batch037 reached the real Rust/C compilers and failed: a temporary cgroup-text
borrow in Rust and old system Landlock UAPI headers. Socket tests were skipped,
not passed. The next source retains the text owner and pins the unmodified
Linux-v6.12 UAPI/license bytes with mandatory CMake hashes and ABI layout checks;
runtime ABI >= 6 remains required. A new exact batch must qualify these changes.

Batch038 source d07305c passed the actual Linux sandbox compilation and all eight
userland socket cases (zero failures/ignored). The subsequent broker check was
blocked by missing dbus development headers in the disposable runner, not a
reported Rust source error at that stage. Bootstrap compilation reached a missing
`sys/wait.h` declaration for WEXITED. Both are addressed in the next source;
bootstrap build and full runtime/broker compilation remain pending new evidence.

Antigravity executes only Codex-released batches on frozen inputs. Codex owns
review, diagnosis, source/test edits and acceptance decisions. No source-copy,
compiler parser or bridge SUCCESS marker marks item 3 or the product complete.
