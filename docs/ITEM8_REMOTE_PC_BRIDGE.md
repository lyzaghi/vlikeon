# Item 8 — Remote Play and installed PC/AAA bridge

The owner selected item 8 after item 7, with practical verification deferred.
No new streaming configuration, relay origin, native input grant or production
feature is enabled by these changes. Authored source is not completion evidence.

## Source changes in this pass

RemoteSession reserves its one connection slot before awaiting ICE credentials.
Duplicate connect attempts cannot create extra peer connections. Offer/answer
and recovery operations each have explicit in-flight admission. Every awaited
offer/answer boundary rechecks the captured peer identity and closed state before
continuing. Offer generations stop at the protocol's actual limit of 16.

Client negotiation has a deadline from the initial credential request. Stop
publishes one shared completion promise before cleanup starts, so repeated and
reentrant callers observe the same bounded teardown. It aborts transport event
subscriptions, retires owned capture, disarms input, detaches peer/channel
handlers, closes the peer and releases track/candidate references. Late incoming
tracks and channels are stopped/closed rather than attached to a closed session.
Optional remote notification and receiver-lease cleanup retain their existing
bounded waits. These do not assert preemption of a blocked browser/OS operation.

Host acceptance now reserves the pending request before publishing capture UI
events. Reentrant acceptance cannot replace or stop the source already owned by
the first call. Closure during a capture callback or sender-parameter update is
checked before continuing. Adaptive bitrate has an explicit close operation:
late parameter completions no longer publish quality events or enqueue another
policy adjustment after session retirement.

Signaling reserves receiver startup before awaiting its persistent lease. Sends
have a 64-task pending bound. Close shares one promise, aborts network work and
waits for an already admitted lease write before attempting release, so that
write cannot renew ownership after a successful release. Store/decryption continuations check
closure before emitting signals or reporting relay connectivity. This does not
claim cancellation of a store transaction or remote operation already admitted.

Installed-game registration deduplicates under an immediate transaction and
records its audit in that transaction. Launch revalidates the PC library
identity, exact executable hash/length and registered working-directory binding.
A launch-request audit precedes process creation; once a real child is retained,
a later diagnostic failure cannot falsely report that launch never happened.

Exit observations retain their original end time and process owner while
accounting retries. An immediate transaction credits playtime and publishes an
exact session-ID `installed.exit_accounted` audit receipt. Retry after an
uncertain commit consults that receipt instead of counting the same session
twice. Only confirmed accounting moves the observation to finished history.
These receipts are accounting evidence, never PID signalling authority; future
audit retention must preserve unresolved accounting receipts.

## Required source and acceptance still open

The subsequent owner request adds an authored Windows owned-window bridge
across Chromium patch 0003, private component-worker binding, broker-owned
process handles, application-only audio and exact-window input admission.
See `OWNED_NATIVE_CAPTURE.md`. This replaces the earlier missing Windows
selection correspondence in source, not in executed acceptance evidence.

The original full streaming scope remains in `STREAMING_PROTOCOL.md` and
`NATIVE_STREAM_INPUT.md`: Linux/macOS owned game/window capture and audio binding,
independent native capture/encode services, remaining input/gamepad integration,
durable credential/mobile lifecycle, safe launcher-child ownership, actual codec
and relay/TURN deployment contracts and any missing device arbitration. Existing
browser capture and optional Windows keyboard/mouse code do not complete all
these implementations. Keep missing source in item 8 until implemented.

Later Antigravity execution must cover simultaneous connect with stalled ICE,
stop before late credentials/capture/offer/answer, repeated-stop promise identity,
pending restart/old peer events, source/track retirement, process launch versus
audit failure, uncertain accounting commits and retained target ownership.
Actual browser/media/network/input/codec/game/platform evidence remains due.
