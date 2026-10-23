# Native H.264/RTP packetization source — item 56

Authored source only. No parser, packetizer, network, decoder, helper, build or
test execution occurred. Eight inert Rust fixture cases are unexecuted. Current
product/WebRTC/receiver code does not invoke this module and streaming stays off.

`platform/streaming/src/native_h264_rtp.rs` admits a backend-delimited access unit
with explicitly selected Annex B or 1/2/4-byte big-endian length prefixes. It does
not auto-detect framing. Input is at most 2 MiB/256 NALs; malformed lengths, empty
NALs, forbidden bits, prepacketized aggregation/FU input and unsupported scalable/
partitioned NAL types are rejected. The supported realtime subset requires IDR
or ordinary non-IDR slice input, never both. NAL header checks do not prove slice
boundaries, valid parameter sets, resolution, profile/level, colour VUI or decoder
compatibility. The actual backend/receiver negotiation adapter must establish
those before packetization/publication. Annex B trailing zero bytes are removed;
the input must begin with a three- or four-byte start code at offset zero.

Packetizer takes admitted RTP-session payload type/SSRC/initial sequence/timestamp,
packetization mode and maximum RTP size. Mode 0 supports only complete single
NAL packets and rejects oversized NALs. Mode 1 preserves NAL order and splits
oversized NALs into FU-A; it does not aggregate/interleave. RTP sequence wraps
normally, timestamps use the 90 kHz clock from capture-relative 100 ns timestamps,
and only the final access-unit packet carries the marker. The initial random
values and SSRC collision handling belong to the real RTP session controller.
Original timestamps must increase and stay within the 30-minute capture bound.

The RTP size bound (256–1400 bytes) excludes outer transport/security/extensions:
the caller must reserve IP/UDP/DTLS/SRTP/tag/extension overhead first. Each batch
has a finite packet budget. Validation/allocation failure does not advance RTP
sequence/presentation state. Successful batches must be delivered complete and
in order under the current media-generation authority, or that generation must
be retired. Dropping arbitrary dependent output is not a backlog policy.

These are plain RTP bytes for an internal transport stage. No socket, SRTP,
DTLS/ICE/TURN, congestion control, pacing, RTCP/NACK/PLI/retransmission, jitter
buffer, receiver/depacketizer or decoder is implemented here. Do not route these
bytes directly to an unauthenticated peer or claim end-to-end streaming. The
browser WebRTC path remains separately implemented; a native injection adapter
and actual profile/level/VUI/parameter-set exchange are still required.

The eight source fixtures cover headers/clock, FU-A reassembly and sequence wrap,
multi-NAL markers, explicit prefix widths, malformed framing, forbidden/nonpicture
input, transactional mode-0 failure and replay/input bounds. They use inert NAL
layout bytes rather than valid decodable pictures and establish no codec/device
compatibility. No tests were executed.

Protocol reasoning follows [RFC 6184](https://www.rfc-editor.org/rfc/rfc6184.html),
sections 5.1, 5.6, 5.8 and 6.2–6.3. That specification does not replace actual
bitstream, decoder, SRTP and network qualification or any original item-56 scope.
