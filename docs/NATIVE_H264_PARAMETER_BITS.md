# Bounded native H.264 SPS/PPS bit reader — item 56

The next source layer, `native_h264_sps.rs`, now parses bounded ordinary SPS and
VUI/HRD metadata using this reader. See `NATIVE_H264_SPS.md` for exact scope. It
does not complete semantic validation, PPS/slice admission or bridge integration;
the remaining requirements below still apply.

Source only. Build/test, parser execution and decoder/network operations remain
deferred by the owner. This is not a `NativeH264Admission` implementation.

`platform/streaming/src/native_h264_bits.rs` accepts one unframed SPS/PPS NAL,
including the ordinary NAL header, with a 64 KiB encoded limit. It checks the
forbidden bit, nonzero reference indicator and SPS/PPS NAL type. It removes
emulation-prevention bytes while rejecting prohibited unescaped byte sequences,
invalid escape followers and empty/trailing-zero parameter payloads. It is not
intended for slice CABAC padding, Annex B splitting or scalable/multiview NALs.

Fixed-width reads have an explicit 32-bit bound and cannot read past the payload.
Unsigned Exp-Golomb reads permit at most 31 leading zeroes and require a semantic
maximum from their caller; signed reads require an absolute maximum. A read
failure poisons the reader rather than letting the caller resume at a different
bit position. Optional-data detection distinguishes remaining syntax from the
final stop/alignment pattern; consuming finish requires the exact trailing bits
without extra bytes. These are syntax primitives, not parameter-set validation.

The implementation follows the NAL/RBSP and Exp-Golomb structure described in
[ITU-T H.264](https://www.itu.int/rec/T-REC-H.264/en), sections 7.3.1, 7.3.2 and
9.1. No specification text or external implementation was copied into the code.

Remaining: complete SPS/PPS/VUI/HRD semantics, parameter ID/reference tables,
profile/level/DPB/crop/colour/timing negotiation, slice/dependency admission,
parameter changes, installed decoder/owner admission, and actual bridge wiring.
No caller currently uses this module to admit a frame. Corpus/fuzz, malformed
input, resource-limit, interoperability and device qualification all remain open.
