# Item 13 — performance, routing and optional assistance

The owner moved this chat from item 6 to item 13, then item 14 on 2026-10-23.
This is a source audit/change record. No build, compiler, browser, measurement,
test or Antigravity batch was run. Item 6 gaps remain in `ITEM6_REMAINING_WORK.md`.

## Source changes

- Local assistant acceptance now reads consent/current settings, checks the
  proposal hash/expiry, writes preferences and records the audit in one SQLite
  transaction. A failed write/audit rolls back the setting. The proposal is
  removed only after successful commit, allowing a failed storage attempt to
  retry while still fresh. Native saved session settings are read with a 4-KiB
  bound. No automatic setting changes or external model execution is introduced.
- Web acceptance rechecks the monotonic expiry inside its existing IndexedDB
  transaction and removes a proposal only after successful commit.
- Web routing validates touch/WebGL/thermal field types and checks the bundled
  core hardware/thread policy rather than relying only on candidate availability.
  Unsupported catalog policy makes a candidate ineligible. Execution retains its
  independent admission checks. No runtime switches during an active game.
- Rust routing rejects invalid minimum-memory bounds and does not use a benchmark
  associated with the unknown `unclassified` device class as device evidence.
  Lowercase pinned revision validation matches other artifact identities.
- Broker-stored Web performance reports identify their platform as `web-worker`,
  rather than borrowing the broker OS/architecture and appearing native. Native
  reports retain actual process target and a distinct measurement source.

## Remaining implementation and evidence

1. Authenticated benchmark inventory bound to exact runtime builds, core configs,
   device/driver identities and applicable games; a controlled compatibility lab.
2. Native/remote registry-backed routing and consent/paired-host availability in
   the common rank endpoint. It currently keeps renderer-supplied native/remote
   candidates unavailable and discards their benchmark claims.
3. Measurement support for brokered presentation routes that currently decline
   measurement protocol, plus platform adapters and meaningful timing counters.
4. Controlled scenes/input traces, repeatable A/B procedures and actual frame,
   latency, power/thermal/device evidence. Interactive reports remain uncontrolled.
5. Any external AI adapter, if later requested, still needs an explicit opt-in and
   the existing bounded proposal boundary. Current assistance is local rules only.
6. Final malformed-input, transaction/race, expiry, routing-policy, capture,
   cancellation and real-platform qualification. Source review is not a pass.

The fixes above do not complete the full item. See `PERFORMANCE_MEASUREMENTS.md`
and `ROUTING_AND_LOCAL_ASSISTANCE.md` for the underlying source contracts.
