# Bounded native RTP ordering — item 56

Source only. No receiver/parser/network/decoder, build or test execution occurred.
Six inert source fixtures are authored, unexecuted. The product has no current
caller for this stage; SRTP/native admission and media publication remain due.

`native_rtp_reorder::OrderedReceiver` connects a finite in-order receive lane to
the H.264 depacketizer. It accepts RTP only AFTER authenticated SRTP decryption
and anti-replay. Bound payload type/SSRC and an authenticated starting sequence
are required; the first arbitrary arriving datagram does not define the capture
generation. Real transport/receiver negotiation must establish that boundary.

At most 64 already-validated RTP packets (each at most 1400 bytes) are held. The
64-sequence forward window uses modular arithmetic, including sequence wrap.
Old authenticated retransmits and equal queued duplicates yield no new frame;
they do not refresh receipt time. Conflicting bytes for an existing sequence,
an excessive forward jump or invalid source/header permanently closes this
generation. The routine is not itself authentication or cryptographic replay
protection and must never be exposed directly to unauthenticated datagrams.

A missing sequence waits at most 50 ms from the oldest buffered packet. tick
must run independently while network traffic stops. Unfilled gaps terminate the
generation rather than sending corrupt dependencies to a decoder. A future real
loss-recovery controller must request fresh keyframe/negotiated generation or
complete authenticated retransmission within the bound; no automatic NACK/PLI,
reset, adaptive playout or loss-concealment guarantee is implemented here.

Ordered packets pass their original local receipt Instant to the depacketizer.
The earliest receipt across all fragments becomes the frame's receipt time,
including a fragment that arrived ahead of the first sequence. Reordering does
not restart the existing 100 ms assembly budget. At most four completed units
are returned per drain (each at most 2 MiB); age is checked again before handoff.
Allocation/error/overflow closes both stages. Already-returned units still need
current generation/authority/codec/decode and final presentation-age checks.

The six source cases cover out-of-order FU-A across sequence wrap with preserved
earliest receipt, duplicate age retention, conflicting duplicate, gap expiry
without new packets, excessive jump/source mismatch and old retransmission.
They use inert layout bytes, not decodable video or an actual network. None ran.

This is one bounded receive-ordering stage, not a complete jitter/playout buffer,
audio synchronizer, SRTP session, ICE/TURN transport, decoder or UI. All original
item-56 service/platform/codec/security/transport/qualification work remains open.

The next source checkpoint adds generic trusted receipt metadata and returns the
earliest original packet metadata with each assembled unit, including through
duplicates and sequence wrap. A Windows source bridge uses opaque decoder
receipts; see `NATIVE_VIDEO_RECEIVE_BRIDGE.md`. One additional metadata source
case is authored, unexecuted; no current network/product caller exists.
