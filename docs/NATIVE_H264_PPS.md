# Native H.264 ordinary PPS metadata — item 56

Source only. Build/test, parsing, decoder/media and network operations have not
run. This module does not grant decoder, session or transport authority.

`platform/streaming/src/native_h264_pps.rs` parses one bounded ordinary PPS NAL
against a supplied SPS with exact ID matching. It retains entropy/field-order
flags, slice-group syntax, default reference-list lengths, prediction weights,
initial quantization, both chroma offsets, deblocking/intra/redundancy flags and
optional 8x8/scaling syntax. SPS bit depth bounds the initial QP domain; optional
second chroma offset defaults to the first when its syntax is absent. Parsing
requires exact stop/alignment bits with no unconsumed bytes.

All ordinary slice-group map types 0–6 have bounded metadata parsing. Run lengths,
rectangle endpoints and change rates are checked against the SPS map-unit count.
Changing maps require two groups. Explicit maps must describe exactly that count
and each group ID must be in range. They allocate at most 262,144 bytes, with
checked allocation, independently of the existing 64 KiB encoded NAL limit.
No FMO macroblock-map expansion or decode support is claimed. Scaling lists
preserve missing/explicit-default/signaled distinctions without deriving fallback
matrices. IDs, group counts, reference counts and all field reads have bounds.

The supplied SPS is public metadata, so arithmetic-relevant geometry/chroma/depth
bounds are checked again rather than assuming it is an untouched parser result.
This is memory/resource admission, not authentication of that SPS's actual bytes
or version. A later retained parameter table must pin those bytes and dependencies.

Syntax and field relationships follow
[ITU-T H.264](https://www.itu.int/rec/T-REC-H.264/en), sections 7.3.2.2 and 7.4.2.2.
This independent source has not passed conformance or interoperability checks.
Remaining: aggregate table quotas/versioning, replacement/invalidation rules,
profile/level/DPB legality, scaling fallback, slices/reference-picture dependencies,
negotiation/backend/native-owner admission and actual receive-bridge wiring.
All malformed/corpus/fuzz/resource/device qualification also remains open.
