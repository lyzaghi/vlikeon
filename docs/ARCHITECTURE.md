# Architecture map

Intended boundaries and authored source, not verified runtime behavior.
See [public status](PUBLIC_STATUS.md).

```text
Chromium / WebKit / web product surfaces
                |
          Game Hub and session UI
                |
    platform contracts and permission broker
          /             |             \
 local library      native runtime     remote-play coordination
 saves / recovery   engine adapters    signaling / stream input
          \             |             /
          platform ownership and containment
                |
       optional account and encrypted sync services
```

Browser requests are intended to pass through explicit contracts and permission
admission. Native workers, selected files, sessions and saves have separate
ownership/lifetime rules. A UI action is not a worker or storage acknowledgement.

Desktop and Android source targets Chromium; iOS source uses WebKit. Shared Rust
code supplies contracts, broker and platform/services. Native adapters and web
workers have different host/memory constraints. Windows, Linux and macOS
containment require separate actual qualification.

Recipes acquire pinned source and apply explicit adaptations. Local generated
snapshots are excluded from Git. A candidate engine is distinct from a component
approved for distribution. See
[`../security/dependencies.json`](../security/dependencies.json).

Early review areas: capability admission, callback retirement, memory/file
bounds, partial failure, save provenance, update trust and dependency licensing.
Read component documents before changing these boundaries.
