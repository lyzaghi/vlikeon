# Native audio final qualification

Authored only. Do not configure, compile or execute before the final item-16
pass. This project uses the production `audio_ring.hpp`, with scripted sinks;
it does not initialize SDL, load a core or establish physical playback.

1. Build the exact source with the qualified Windows x64 C++20 toolchain, then
   execute `omniplay-native-audio-cases` and CTest `native-audio-ring-contract`.
   Record toolchain, source/artifact hashes and output. In the frozen source
   project set `OMNIPLAY_NATIVE_AUDIO_SOURCE` to `source/native/src` explicitly.
   Cover partial/full admission, failed sink retention, no slot reuse before
   commit, intentional overload trim, wrapped sample order, priming/silence,
   signed volume conversion, rejected input, quiescent reset and concurrent
   producer/consumer order. Use a supported race detector on applicable targets.
2. Execute Windows acceptance case 24 with actual workers and every affected
   core. Inject SDL failure both before any copy and after an internal queue
   prefix copy. Require a latched runtime failure, pending SDL queue clearing,
   no later consumer replay or successful resume, and real process retirement.
   Retained source-ring bytes are not proof that SDL copied nothing. Already
   submitted hardware audio cannot be retracted by clearing the stream.
3. Inject stream lock, clear, format, unlock, pause and resume failures. Failed
   pause must still attempt the guest producer barrier. Failed resume must not
   publish running/started state. Repeated resume while playing must not reset
   the ring. Exercise normal rate changes, save/load and pause/close flushes;
   verify reset only after producer quiescence and under the SDL stream lock.
4. On the actual presentation client, inject allocation failure at each packet
   body/list node and expire focus/change epoch while the private batch is
   staged. Also pause or clear stale-producer media without changing the host
   epoch, then finish old audio/video copies: neither may repopulate that cleared
   generation. Require no prefix from rejected batches on the wire and unchanged
   pending rate/bytes/packets from that rejected call. Race the writer against
   commit and observe the whole admission boundary. Independently qualify
   oldest-packet trimming, rate changes, EOF and post-commit transport failure.
   A true queue receipt is not a helper or device playback acknowledgement.
5. Preserve final saves and broker ownership across all above failures. Qualify
   latency, underrun accounting, device loss/reconnection, real controller/audio
   hardware, single-sample callbacks and core retry policies. These authored
   cases do not close the complete Windows or graphics/audio/input requirements.
