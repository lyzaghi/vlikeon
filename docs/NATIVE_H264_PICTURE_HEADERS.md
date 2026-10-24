# Native H.264 primary-picture header consistency — item 56

Source only; build/test/parser/decoder/media/network execution remain deferred.
`native_h264_picture.rs` checks one primary picture's ordinary slice headers in
an already bounded `AccessUnit`. The framing layer exposes immutable borrowed
NALs; it does not gain codec admission or mutate the parameter table.

Every ordinary slice resolves the exact retained parameter versions. Headers
must agree on picture identity, reference marking, changing-FMO cycle and inferred
SP-switch state. Duplicate first-macroblock addresses, missing macroblock-zero
start, inconsistent same-kind-required flags and inconsistent IDR layout fail.
Arbitrary slice order is preserved rather than silently sorted. In-band SPS/PPS
must be byte-identical to the versions used by the slices; unrelated/replacing
parameter NALs require an explicit preceding parameter coordination step.

This single-primary-picture primitive refuses redundant pictures and separate
colour-plane pictures rather than dropping or mixing them. Supporting those
requires a multi-picture AU coordinator; no product policy silently narrows the
negotiated codec on their behalf. SEI/AUD/end/filler payload semantics remain
unvalidated. Consistent headers do not establish macroblock coverage/nonoverlap,
entropy correctness, reference existence, output-order validity or decodability.
The returned type is explicitly header metadata, not a decoder/session permit.

Picture/header rules are based on
[ITU-T H.264](https://www.itu.int/rec/T-REC-H.264/en), sections 7.4.1.2 and 7.4.3.
Remaining: complete AU coordination and auxiliary NAL validation, DPB/reference
tracking and recovery, negotiated profile/level/colour/timing/backend admission,
native authority and actual receive-bridge wiring. Corpus/fuzz/fault/interop and
device qualification remain open. No caller currently uses this to submit media.
