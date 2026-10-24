# Native H.264 ordinary slice-header syntax — item 56

Source only. No parser, compiler, test, media/decoder/device or network operation
ran. This is not entropy decoding, complete AU admission or DPB validation.

`native_h264_slice_header.rs` continues the retained-parameter-bound slice prefix
through ordinary unpartitioned slice-header syntax. It records B direct mode,
active reference counts, bounded list modifications, explicit prediction weights,
reference-picture marking/MMCO, CABAC initialization, QP/QS, SP switching,
deblocking and changing-FMO cycle. Frame/field reference limits, list terminators,
weight domains, quantization/deblocking domains, cycle bounds and local MMCO
duplicate/reset restrictions are checked. Missing/default weights remain distinct
from explicit values. Implicit weighting and list/DPB derivations are not performed.

The original lazy reader and 4096 encoded-header-byte budget are retained. List
edits cannot exceed their active list count (at most 32); adaptive marking has a
64-command resource limit and requires its terminator. The reader counts RBSP
bits and checks one-bit CABAC alignment after the header when signaled. Reported
bit counts exclude the NAL header/emulation bytes and must not be interpreted as
byte offsets into the original encoded NAL. Header completion does not establish
the presence or validity of entropy data, end padding or trailing bits.

The syntax is independently implemented from
[ITU-T H.264](https://www.itu.int/rec/T-REC-H.264/en), sections 7.3.3/7.4.3 and the
CABAC alignment rule in 7.3.4/7.4.4. No copied decoder implementation is included.
No conformance corpus or interoperability result has validated this source.

Remaining: DPB state, actual short/long-term reference existence and list edits,
ordered MMCO effects/constraints, IDR/MMCO5 recovery, field/MBAFF/reference rules,
same-picture/multislice coverage, entropy/trailing validation at the decoder,
profile/level/colour/timing negotiation, parameter changes, authenticated native
owner/decoder/bridge integration and all malformed/fuzz/device qualification.
