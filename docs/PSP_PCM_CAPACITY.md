# PSP PCM destination contract

This is authored source for the `sw3`, `gl3` and `websw3` candidates. No compiler,
decoder, core, browser or device has been executed. The common source adapter is
`tools/ppsspp_audio.py`, applied after the pinned FFmpeg 9 adaptation.

## Sized output

`AudioDecoder::Decode` takes the destination capacity explicitly, in per-channel
sample frames. A stereo destination with capacity 1,024 therefore needs 4,096
bytes. `outSamples` is output-only; it cannot be used to guess physical storage.
The interface has no unsized overload or default capacity. A null destination
with zero capacity discards PCM and can report decoded frames without writing.
All selected public callers pass the capacity of their actual buffer or proven
guest-memory span. Failure clears the output count before a caller can publish it.

The FFmpeg converter admits one or two input/output channels, valid channel
layouts, sample rates from 1 to 384,000 Hz and at most 8,192 frames. It writes
through private storage of 8,192 stereo signed-16-bit frames. It commits only the
validated returned frame count to the caller's destination. A rejected conversion
does not copy a partially written conversion into guest memory. This does not
make other upstream decoder or HLE state changes transactional.

The converter caches the actual input layout, sample rate, sample format and
output channel count. A changed key builds a separate initialized context before
retiring the previous one. Discard, flush and destruction retire buffered
conversion state. Pending output cannot silently exceed the supplied capacity.
This is a source-level limit, not a measured process memory or wall-time bound.

FFmpeg uses its selected channel-layout conversion; the standalone ATRAC backend
retains its left-channel mono policy. Exact PSP mixdown, amplitude, delay and
cross-backend equivalence need real fixtures during final qualification.

The factory retains the pinned parent's backend choice: ATRAC normally uses its
standalone decoder, while MP3/AAC and the parent's FFmpeg choice use the new
FFmpeg path. The capacity contract covers both. Updating FFmpeg therefore does
not establish that the separate standalone ATRAC fork has been modernized or
qualified; its maintenance and a complete replacement policy remain open.

## Call sites

ATRAC ordinary and low-level paths use their full 1,024/2,048-frame hardware
buffer shape. SAS uses a bounded mono destination. The standalone ATRAC backend
also applies the explicit capacity and rejects an output beyond its supported
2,048-frame shape. MediaEngine has 2,048 stereo frames. AuCtx derives its stereo
capacity from its actual PCM half-buffer. MP3 low-level calls have 1,152 stereo
frames. The audiocodec HLE chooses its codec-specific frame count before resolving
a complete writable guest-memory span. Low-level metadata outputs require their
full four-byte spans as well.

Retained ATRAC temporary storage covers the largest admitted later frame/channel
shape, including changes to a guest-owned context. A short decode cannot publish
a larger requested count or copy an unproduced temporary tail. The legacy ATRAC
path trims decoded samples using its actual decoded count and skip offset. SAS
packet assembly checks both readable parts and address/offset arithmetic before
copying; AuCtx checks the full double buffer, its half selector, input/count
bounds and metadata span before using the selected half. Nonfinite standalone
float output becomes silence before signed-integer conversion.

These bounds deliberately reject malformed declarations. They do not certify
compatibility with every game, packet boundary, damaged state, allocation failure
or address alias. The final qualification must inspect the actual compiled
decoder and every remaining upstream raw-pointer boundary.

## Product and evidence identity

The new candidate core versions distinguish the changed audio API and output
behavior. In-game file formats remain `ppsspp-files-opsf2` on native profiles and
`ppsspp-websw1-files` on Web. Opaque snapshots still require their exact core and
configuration identity; Web declares `ppsspp-websw3-state`.

Native build receipts declare `audio_output_protocol: 1`, and native package
admission requires that declaration together with the selected decoder receipt.
The Web corresponding-source inventory includes the PCM adapter, converted
sources and private helper. Historical source trees and receipts stay retained.
Source preparation is not a binary, license, containment or distribution approval.

The `-v3` native software/OpenGL and Web preparations are complete. The Web
inventory contains 22,080 source files. The source-only authoring refinement
retains original changed files and receipts under
`out/authoring-history/ppsspp-pcm-bounds-v1`; each project identifies the exact
before/after hashes in `source-authoring-transition.json`. The Web build's
auxiliary inventory requires that record in addition to its source/recipe hashes.
This refinement also propagates failed frame/context allocation and failed codec
opens without marking the decoder open or dereferencing the absent resources.

The single final consolidated verification remains responsible for all supported
codec/rate/channel transitions, mono/stereo switching, seek/flush/discard, short
and exact destination spans, canaries, malformed inputs and guest contexts,
decoder/resampler allocation failures, delay/drain behavior, count publication,
thread/stop races, save compatibility and real-device latency/memory measurements.

This change does not complete upstream compressed-input queue limits, decoder
construction/allocation failure propagation, every retained guest-context lifetime
or final source/binary provenance qualification. The newer committed native and
atlas source producers are described in `PSP_SOURCE_PROVENANCE.md`; their source
inventories do not qualify a binary. The remaining source obligations stay open
alongside the rest of the product and must be closed before release qualification.
