# Pinned Linux userspace API

`linux/landlock.h` is an unmodified copy from Linux v6.12, commit
`adc218676eef25575469234709c2d87185ca223a`, upstream path
`include/uapi/linux/landlock.h`. `source.json` records source paths and SHA-256.

Upstream: https://github.com/torvalds/linux/tree/adc218676eef25575469234709c2d87185ca223a

The header retains its `GPL-2.0 WITH Linux-syscall-note` identification and
copyrights. The matching upstream license and syscall exception are preserved
in `licenses/`. They apply to these third-party files independently of this
repository's original Apache-2.0 source. Binary distribution and static libc
license/relink obligations still require their separate release review.

CMake verifies exact source/license hashes. Git preserves LF bytes on all
platforms. The policy includes this header explicitly, checks its structure
layout at compilation and still requires runtime Landlock ABI >= 6. This is
not a fallback policy for a kernel missing the required rights or scopes.
