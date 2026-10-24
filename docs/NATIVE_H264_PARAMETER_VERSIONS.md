# Retained native H.264 parameter versions — item 56

Source only. No compiler, parser, test, decoder/media/device or network operation
ran. The actual negotiated bitstream admission and receiver caller remain open.

`native_h264_parameters.rs` owns encoded SPS/PPS NALs and metadata parsed by the
existing bounded parsers. There is no constructor accepting caller-supplied
parsed structs. Resolved metadata and exact encoded bytes are immutable borrows.
PPS parsing selects an already retained SPS by its actual signaled ID and records
the exact SPS version. Resolving a PPS requires that dependency still match.

An exact encoded repeat is idempotent and preserves its entry version. An SPS
replacement advances the table's bounded monotonic version and invalidates every
PPS referencing that SPS ID. Even identical old PPS bytes must be resent and
reparsed against the new SPS version. Replacing a PPS advances its version.
Unrelated parameter updates do not invalidate an unchanged dependency pair.

Opaque binding objects retain a table-local allocation identity plus both entry
versions. Checking a binding rejects another table, replacement, missing PPS/SPS,
or a closed table. Close clears the table and cannot reopen that generation;
existing binding objects cannot accidentally match a newly allocated table.
Bindings are metadata-version handles, never native decoder/session permits.

Quota preflight occurs before replacements/dependency removals: at most 32 SPS
IDs and 256 PPS IDs, 1 MiB aggregate encoded NAL bytes, 1 MiB explicit slice-group
map entries and a 2 MiB accounting budget. Accounting charges 4096 bytes per entry
plus encoded/dynamic data, including POC/scaling/HRD arrays; this is a conservative
logical quota, not a measurement of allocator/OS resident memory. Parsing/copying
one incoming bounded NAL and its metadata temporarily requires additional memory.
Returned parse/quota/copy errors preserve the previous table and version. Version
exhaustion requires a new native media generation rather than wrapping counters.

Remaining: actual AU/slice parameter selection and IDR/reference-picture recovery,
queued-frame invalidation and parameter-change negotiation, profile/level/DPB/
colour/timing/backend admission, final native owner checks and bridge wiring.
Parameter replacement is not itself permission to continue decoding dependent
pictures. Corpus/fuzz/fault/resource/interop/device qualification remains open.
