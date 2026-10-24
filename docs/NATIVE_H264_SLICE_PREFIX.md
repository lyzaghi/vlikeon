# Native H.264 ordinary slice identity prefix — item 56

Source only. No parser, compiler, tests, decoder/media/device or network operation
ran. This is not a complete slice header, reference validator or decoder permit.

`native_h264_slice.rs` reads a bounded identity prefix from ordinary unpartitioned
NAL types 1/5. It resolves the actual PPS ID through retained parameter versions,
then reads colour plane, frame number, field/bottom-field flags, IDR ID, POC
prefix and optional redundant-picture count according to that exact SPS/PPS.
The first macroblock address is checked against picture size including field
and MBAFF interpretation. IDR requires a reference NAL, zero frame number and
I/SI slice type. Zero-reference SPS also requires I/SI prefix type. Slice types
0–9 retain their normalized kind and same-kind-required indication.

The internal reader borrows the NAL, limits total NAL size to 2 MiB and encoded
prefix consumption to 4096 bytes, and only unescapes bytes as reads require them.
It never scans the whole entropy payload to find a prefix. Reads have sticky
failure, explicit bounds and finite Exp-Golomb prefixes. It does not validate
unread body bytes, CABAC padding, trailing syntax or full header/data completion.
Thus even a successful prefix may belong to a truncated or undecodable slice.

Picture identities include a table-local exact parameter binding, frame/field/
plane, reference/nonreference distinction, IDR ID, POC values and redundancy.
Parameter binding equality compares allocation identity and versions, not merely
equal IDs or the value of the identity marker. Stale/cross-table bindings fail
the existing table check. Comparing identities is a grouping prerequisite only;
it does not prove complete picture coverage, a complete access unit or valid
reference-picture dependencies. Different nonzero nal_ref_idc values are retained
on the prefix but share the reference identity distinction.

Prefix syntax is based on
[ITU-T H.264](https://www.itu.int/rec/T-REC-H.264/en), sections 7.3.3 and 7.4.3.
Remaining: remaining header/reference-list edits/weights/marking, CABAC/QP/filter
fields, primary/redundant/multislice coverage, IDR recovery and DPB/reference
tracking, parameter-change and profile/level/colour/timing admission, authenticated
native owner/decoder integration, and corpus/fuzz/device/fault qualification.
