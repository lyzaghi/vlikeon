# Native H.264 ordinary SPS metadata — item 56

The subsequent PPS metadata parser is documented in `NATIVE_H264_PPS.md`.
It does not complete parameter versioning, semantic/negotiation checks or slice
dependencies; the broader remaining requirements below still apply.

Source only. No parser, compiler, test, decoder/media/device or network operation
ran. The receiver admission interface still has no complete implementation.

`platform/streaming/src/native_h264_sps.rs` parses one ordinary SPS through the
bounded parameter-set reader. It retains profile/constraint/level fields, SPS ID,
chroma format/separate planes/bit depths, transform bypass and scaling-list
syntax, frame-number/POC modes, reference count/gaps, coded geometry, field mode,
direct-8x8 and a chroma/field-unit-derived visible crop rectangle. Missing scaling
lists, explicitly selected default matrices and signaled values stay distinct;
fallback matrix derivation is not performed.

VUI metadata retains absent versus explicit aspect ratio, overscan, video range
and colour identifiers, chroma location, timing, both HRD sets/delay lengths,
low-delay/picture-structure flags and bitstream restrictions. CPB quantities use
bounded 64-bit arithmetic; frame-rate values remain rational timing metadata,
not a rounded or inferred negotiated FPS. Colour IDs are retained without
inventing a default colour contract or claiming HDR support.

Resource/syntax boundaries: 64 KiB encoded NAL, SPS ID 0–31, at most 12 scaling
lists of 64 entries, 255 POC-cycle offsets, 16 reference/buffering frames and 32
CPB entries per HRD set. Coded dimensions are bounded to 8192 per axis and empty
or out-of-range crop rectangles are rejected. Reads and loops use explicit
limits; complete trailing syntax is required. Ordinary baseline/main/extended
and explicitly listed high-family syntax is recognized. Unknown profiles/levels,
reserved syntax, unsupported larger dimensions, and signed Exp-Golomb extremes
outside the bit reader's bound fail rather than being guessed.

The syntax is based on [ITU-T H.264](https://www.itu.int/rec/T-REC-H.264/en),
sections 7.3.2.1, 7.4.2.1 and Annex E. This independent source implementation has
not been checked against a conformance corpus. A parsed SPS is public metadata,
not validated decoder/session authority. Profile-specific constraint legality,
level/DPB/rate capacity, PPS/slice dependencies, scaling fallbacks, colour/timing
negotiation, parameter changes, actual backend/owner admission and bridge wiring
remain outstanding, together with all malformed/corpus/fuzz/device qualification.
