# Pinned modern PSP decoder source

Source preparation has completed for `out/decoder-sources/ffmpeg-web-9.0.2`,
`out/web-cores/ppsspp-v2`, `out/native-sources/ppsspp-v2` and
`out/native-sources/ppsspp-gl-v2`. Their source receipts and current makefile
inputs are retained. Earlier source selections are preserved separately and
do not count as compiler, runtime or test results. No binary artifact was built.

The current PSP native software, native OpenGL and Web software producers use
FFmpeg 9.0.2, commit `946fcce07b6dcd0331c8cc609192aeff5e1924f8` and annotated
tag object `ce8f11b9fac4cd7ccfb41ca7c01bc17f5cda8cc3`. The official download
page dates this release to 18 September 2026. The selected mirror is listed by
the project itself. [Release/source information](https://ffmpeg.org/download.html).

The parent PPSSPP commit and its original gitlink lock are retained. The product
explicitly overrides the parent's old `ffmpeg` gitlink with this separate pinned
source. Its lock records that tag-signature, compiler, runtime and distribution
qualification have not run. A version/date or successful source fetch is not
evidence that a particular compiled product is secure. Actual advisory mapping,
component selection, fuzz/containment/device evidence and release review remain
required. [Upstream security information](https://ffmpeg.org/security.html).

## Source and build separation

`fetch_ffmpeg_sources.py` fetches the exact official tag and checks its object and
peeled-commit identity. It preserves a different existing checkout. It does not
verify the tag signature, install binaries or execute project scripts. The PSP
source fetcher keeps its original parent lock while selecting the current decoder
by default. `--include-parent-ffmpeg` additionally retains the old parent source
for archaeology; it does not enable an old decoder in the product build.

`ffmpeg_source.py` materializes committed source through bounded Git archives.
Selection includes every unconditional root-makefile source dependency, including
`tools`, `fftools`, documentation and test makefiles. These files are build inputs
only; no tool, example or test is executed during preparation. Its source
inventory, selected-path list, original notices and archive hash are retained.

`build_ffmpeg.py --prepare-only --target web` authors a decoder source project
under `out/decoder-sources/ffmpeg-web-9.0.2`. Native target selections are
`windows-x86_64` and `windows-arm64`. Actual configure/make/compiler/install
commands require `--build-prepared` and explicit real tool paths. Compilation is
deferred under the user's author-all-then-one-verification policy.

The finite configuration enables H.264, AAC, ATRAC3/ATRAC3+ and MP3 decoders;
MPEG-PS/TS, OMA, MP3 and AAC demuxers; and H.264/AAC/MPEG-audio parsers. It
disables component/external-library autodetection, all URL protocols, networking,
encoders, muxers, hardware acceleration, programs, GPL/nonfree/version-3 options
and assembly in this source baseline. Its library-only commands do not request
examples or tests. Platform/thread flags are explicit.
Optimized codec paths and their actual performance/security inventory still need
separate reviewed implementation and qualification.

The Web build requests the selected SDK's pthread/C-exception ABI. Windows
profiles request clang-cl/MSVC-compatible static CRT libraries, with explicit
architecture and toolchain paths. This is an authored recipe; it does not prove
actual compiler acceptance, CRT/ABI compatibility or target performance.

A compiled decoder receipt identifies the exact target, library/header/product
hash inventory, source/recipe/lock hashes, compiler-tool hashes, configuration and
retained notices. It stays unapproved. Current core build commands require this
compiled receipt; a source-only receipt or an old parent library is rejected.
The native CMake adapter imports only the explicit verified prefix instead of
running the old bundled FFmpeg source-list generator. Core packaging requires
the matching modern decoder identity. Final signed artifact provenance and actual
binary/source correspondence remain separate release obligations.

## Caller adaptation

The shared `ppsspp_ffmpeg.py` adaptation selects const codec descriptors for the
modern API and supplies a static helper for finite decoder contexts. Every
selected decoder open requests one codec thread, at most 1920×1080 pixels and
8,192 audio-frame samples. The old `threads=0` dictionary is replaced with `1`:
otherwise `avcodec_open2` could apply it after context fields and undo the bound.

AVC and audio packet ownership uses `av_packet_alloc`/`av_packet_free` with RAII.
Borrowed input is not freed as an owned buffer. Audio input uses an owned padded
packet; AVC input and parser probes use explicitly zero-padded buffers. Packet
input is bounded at 16 MiB before allocation/copy. These adaptations avoid relying
on deprecated stack packet initialization in the changed caller paths.

Custom-AVIO source admission checks the I/O-buffer allocation and context
allocation independently. It declares at most 64 streams, 512 probe packets and
2,000,000 microseconds of media-analysis duration. That duration is not a wall-
clock execution deadline. Failure paths release newly allocated codec contexts.
Format close precedes freeing the current custom-AVIO buffer/context, respecting
FFmpeg's ability to replace the input buffer. An unsuccessful format open also
retires its retained custom-I/O resources before another attempt.

Native save barriers now drain pending drawing, savedata-dialog I/O and the
separate sceIo manager. Web keeps its explicit joined pthread barrier. Normal
guest pause/resume/context ownership still follows each selected software/GPU
profile. No concurrent producer, destructor, allocator, parser/decoder or file
behavior has been executed or qualified.

## Product/save identities

Native versions advance to `1.20.4+omniplay.sw3.a5275067b624` and
`1.20.4+omniplay.gl3.a5275067b624`; the Web version advances to
`1.20.4+omniplay.websw3.a5275067b624`. Current native directories use
`-v3-committed`, and Web uses `-v3-committed-atlas`, with the separate atlas
producer in `PSP_SOURCE_PROVENANCE.md`. Historical source projects remain
preserved. No immutable saved record is
rewritten to pretend it came from the new decoder.

The native in-game `ppsspp-files-opsf2` contract and Web in-game
`ppsspp-websw1-files` plus its existing file envelope remain unchanged. Decoder
replacement does not invent a different persistent-file wire format. Opaque
snapshots retain exact core version/build/configuration requirements, and the
new Web snapshot declaration is `ppsspp-websw3-state`. Native/Web cross-platform
progress conversion and any historical snapshot migration remain separate work.

## Final evidence still due

No source-preparation result constitutes a compiler, codec or security test.
The final consolidated pass must cover tag signatures, all selected/generated
make/config inputs, native/Web compilation and ABI/CRT/SDK compatibility; actual
compiled codec/notice/source inventory; H.264 reorder/drain, parser timing,
ATRAC/MP3/AAC/channel/layout/resampling behavior; invalid/short/huge input and
decoder outputs; the sized destination contract in `PSP_PCM_CAPACITY.md` and all
remaining upstream raw-pointer boundaries; allocation/failure
cleanup and custom-I/O ownership; thread limits and forced cancellation; saves,
rollback and GPU context lifetime; device RSS, latency, energy and thermal limits.

The parent retains a pinned private libavformat parsing hook; this is not a
forward-compatible public API promise. All other original browser/runtime/mobile,
PC capture/streaming, sync, containment, licensing/store/release requirements
remain open. No artifact or requirement-completion evidence is promoted here.
