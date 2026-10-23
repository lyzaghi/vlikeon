# Private Windows encoder candidate — item 56

Source only. No CMake configure/build, encoder activation, COM/MF call, frame,
GPU/device operation or test was executed. No product capability is enabled.

This standalone static-library recipe supplies an H.264 Main Level 4.2 NV12
Media Foundation adapter for 720p/1080p at 30/60 FPS. It enumerates system MFTs
with bounded activation attempts and admits a single input/output stream. A
hardware-required request uses hardware-MFT enumeration and checks the activation
classification; this is not proof of actual GPU use or hardware performance.

The admitted host must own a dedicated COM MTA helper thread, call create there,
then submit/poll/take/stop/destruct on that same thread. Poll at least every 8 ms
while active. Async MFTs are unlocked and consume NeedInput/HaveOutput credits;
sync MFT output is pulled until NeedMoreInput, with finite loop counts. Event
polling uses NO_WAIT, but transform/COM/driver calls can still block. This library
does not supply a process supervisor, sandbox or timeout preemption.

Input is tightly packed NV12 copied into an alignment-admitted MF buffer. At most
two accepted frames are pending. Checked low-latency/no-B-frame codec settings
and exact output timestamp/keyframe receipts establish the candidate's required
one-input/one-output contract. An incompatible MFT fails rather than silently
changing the contract. Input setup/encoding age is measured from before the raw
copy, never from the return of a slow ProcessInput call.

Encoded output is owned bytes, up to four packets/4 MiB total and 2 MiB per packet,
plus bounded sequence headers included in the same total budget. Overflow,
unexpected timestamp/format/event, missing requested keyframe, expired grant or
100 ms processing/output age permanently closes delivery. No dependent encoded
frame is arbitrarily evicted. Unexpected stream changes require future explicit
reconfiguration and negotiation; they currently terminate the candidate session.
Packets are backend-native H.264 data, not a promise of RTP or Annex-B framing.
The actual media type and sequence headers must be admitted by the future
receiver negotiation adapter before any publication.

The host must bind the instance to its original capture/process/document/pair
authority and check it before submission and publication. close_delivery is a
thread-safe immediate queue fence; an already-taken Packet still belongs to the
consumer and needs the same publication fence. stop separately flushes/ends
streaming and requests activation shutdown. A failed stop retains references in
the live Encoder and returns failure; the future service retains that object and
reports cleanup uncertainty. Destructor cleanup is best effort, never a receipt.
The original grant deadline is not renewed by input, polling or codec operations.

Remaining: actual independent service executable/private transport/supervision,
owned Windows capture surfaces and BGRA/GPU-to-NV12 conversion, application audio,
DXGI-only MFTs, additional negotiated codecs, product packaging/admission, real
decoder/receiver integration, provider provenance and full build/runtime/fault/
latency/energy qualification. See [the item-56 scope](../../docs/NATIVE_VIDEO_SERVICE.md).

API source review used Microsoft documentation for
[asynchronous MFTs](https://learn.microsoft.com/en-us/windows/win32/medfound/asynchronous-mfts),
[H.264 encoding](https://learn.microsoft.com/en-us/windows/win32/medfound/h-264-video-encoder),
[low-latency mode](https://learn.microsoft.com/en-us/windows/win32/medfound/codecapi-avlowlatencymode),
[keyframe requests](https://learn.microsoft.com/en-us/windows/win32/medfound/codecapi-avencvideoforcekeyframe)
and [buffer alignment](https://learn.microsoft.com/en-us/windows/win32/api/mfapi/nf-mfapi-mfcreatealignedmemorybuffer).
Installed Windows SDK headers were read to verify symbol declarations; this is
not compiler, API execution or device evidence.
