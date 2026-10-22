# PSP OpenGL native source profile

The current `gl3` producer uses the pinned FFmpeg 9.0.2 caller/import adaptation
and joined savedata barrier in `PSP_DECODER_MAINTENANCE.md`, plus the bounded PCM
destination contract in `PSP_PCM_CAPACITY.md`. Its source directory
has the `-v3-committed` suffix and uses the committed-source producer described in
`PSP_SOURCE_PROVENANCE.md`. Its explicit prepared build requires the current compiled decoder
receipt. The earlier `gl1` source description below is retained as historical
context; no old snapshot is relabeled as `gl3`, and no profile has been qualified.

`tools/build_ppsspp.py --profile opengl --prepare-only` materializes an adapted PPSSPP source tree at the same immutable parent and thirteen dependency pins as the software profile. Its identity is `ppsspp-gl-native`, core version `1.20.4+omniplay.gl1.a5275067b624`. The signed worker/backend declaration is `opengl_core`, with a 480×272 private render target and one-times internal resolution. Both profile source preparations have completed; no CMake configuration, compilation, driver/context creation or emulator execution has occurred.

The selected CPU path is upstream `IR_INTERPRETER`, whose IR cache stores interpreted bytecode. It is not a native CPU JIT. Vertex JIT capability remains false, networking options remain disabled, and the Windows AppContainer/executable-heap policy is retained. Other upstream GPU/JIT implementation files remain compiled inputs and must be included in the actual source/license inventory. This path does not complete the product's native JIT or mobile runtime requirements.

The core factory selects only its OpenGL core 3.3 context and propagates creation/preset failures. The shared native worker requires the compiled backend to match the admitted manifest and checks robust-context/reset support. Content loading may begin asynchronous upstream boot, but Ready is delayed until the host has reset the graphics context and the exported `omniplay_finish_prepare` has completed `PSP_InitUpdate` successfully with valid memory/GPU state. This completion hook uses a monotonic 30-second loop bound; the broker retains its independent preparation deadline and process termination. No guest frame, input or emulation-thread start is issued by the completion hook. A blocking driver call is contained by the outer process deadline, not the inner clock loop.

The signed `pause_barrier` declaration requires an exported guest pause hook. Pausing parks/drains the emulation/render queues before resetting the host's audio ring, flushing files or taking snapshots. Adapted serialize/unserialize functions leave the guest paused; only a later `retro_run` owns emulation-thread resumption. Shutdown joins guest producers before destroying the audio sink and unloads the core while its graphics context is still alive. Restart after a file restore repeats context reset and boot completion.

Audio/geometry updates are queued by callbacks and consumed on the host thread. Clock/sample-rate changes park the guest first, merge any final queued update, reconfigure the audio stream and restart the host pacing clock. Valid clocks include PSP's approximately 20 Hz mode rather than rejecting fractional values below 20. Callback fault/shutdown notifications use atomic state; shutdown during boot is a content failure, while a live guest shutdown becomes a paused close/save request. These source-level lifecycle rules remain unverified against real threaded behavior.

Both PSP profiles share the bounded persistent-file codec in `PSP_SAVE_FORMAT.md`. In-game files may use the explicitly declared portable reader relation; snapshots retain exact core/build/backend/options/asset identities. Open-font atlas generation is required before a product core build. PGF asset provenance, compiled component inventory and complete corresponding source still prevent distribution approval. No proprietary font, firmware or user game has been added.

The final consolidated pass must cover compilation/imports, actual artifact content/notices, boot cancellation and failure, frame/audio delivery, preset/clock changes, pause and stop races, state rejection and file rollback, robust-context loss, DPI/fullscreen, input, formats and real performance on target devices. No compatibility, speed, latency, battery or release claim is established by this source profile.
