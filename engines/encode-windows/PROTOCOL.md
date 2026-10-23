# Private Windows video worker protocol v1 — authored, unexecuted

The CMake recipe defines `omniplay-video-worker.exe`. It runs only with the exact
argument `--stdio-video-v1` and pipe handles for stdin/stdout. It accepts owned
raw video bytes, not a display/window token or capture authority. No file/network
destination, renderer RPC, arbitrary path/CLSID or permission prompt is accepted.
Installed-helper admission, launch job/resource limits, original process witness,
independent supervision and broker integration remain required before product use.

All integers are little-endian. Each frame has a 20-byte header:

| Offset | Size | Value |
| --- | --- | --- |
| 0 | 4 | ASCII `VLKV` |
| 4 | 2 | Protocol version 1 |
| 6 | 2 | Message kind |
| 8 | 8 | Request ID |
| 16 | 4 | Payload byte length |

Requests start at ID 1, increase exactly by one and stop at 10,000,000. The parent
serializes calls; at most two complete commands/responses queue. Oversized/unknown
frames are rejected before payload allocation. Short, malformed, reordered or
overloaded input is terminal. Ordinary failure can produce an HRESULT status or
EOF; both fence the generation. No stdout logging is permitted.

## Configure — kind 1, exactly 61 bytes, first request only

| Payload offset | Size | Value |
| --- | --- | --- |
| 0 | 16 | Nonzero capture UUID bytes |
| 16 | 16 | Nonzero session UUID bytes |
| 32 | 8 | Generation 1–1,000,000 |
| 40 | 4 | Width 1280 or 1920 |
| 44 | 4 | Matching height 720 or 1080 |
| 48 | 4 | FPS 30 or 60 |
| 52 | 4 | Bitrate 500,000–30,000,000 bps |
| 56 | 4 | Remaining lifetime 1–1,800,000 ms |
| 60 | 1 | Require registered hardware MFT, 0 or 1 |

UUIDs are immutable private labels, not native consent or proof of live game,
process, pair or document ownership. The installed broker supplies that authority.
The original lifetime starts at header receipt and includes transfer/setup.
Successful Configure returns kind `0x8000`, matching request ID and 66 bytes:
4-byte HRESULT, exact 61-byte Configure echo, then a one-byte registered-hardware
MFT classification. It is not measured GPU evidence. The backend requests H.264
Main Level 4.2; the actual bitstream/type still requires receiver admission.

## Frame — kind 2

Payload: nonnegative signed-64-bit timestamp in 100 ns units (8 bytes), forced
keyframe flag 0/1 (1 byte), then width*height*3/2 tightly packed NV12 bytes.
The byte contract is SDR BT.709 primaries/transfer/matrix, limited range and
centered 2x2 chroma. Full-range, unknown/HDR or differently sited NV12 requires
explicit conversion first. MF requests these colour fields; actual coded VUI
and receiver colour still require validation before publication.
Maximum complete payload: 3,110,409 bytes. Queued commands older than 100 ms are
rejected. Encoder processing age separately starts before its raw copy. Timestamps
increase across accepted frames. Reply `0x8000` contains a 4-byte HRESULT: S_OK
means accepted, S_FALSE means raw input was not accepted and may be discarded;
failed HRESULT/EOF terminates the generation. Frame never returns encoded media.

## Poll — kind 3, empty payload

Services encoder events and takes one output packet. Empty returns `0x8000` with
S_FALSE. Packet reply `0x8001` contains timestamp (8 bytes), keyframe flag (1),
encoded length (4), sequence-header length (4), then encoded and header bytes.
Encoded data is at most 2 MiB, header at most 64 KiB; the Encoder's four-packet/
4 MiB total budget includes headers. This is backend-native H.264 framing, not an
RTP or Annex-B guarantee. Dependent encoded packets are never evicted for backlog.
An encoded reply waiting on the writer for 100 ms closes transport instead of
dropping a dependent packet and continuing. A write already blocked inside the
OS still requires parent-side timeout/cancellation and a late-reply fence.

## Stop — kind 4, empty payload

Terminal even before Configure. Reader receipt supersedes unsent ordinary replies.
The actor fences delivery before MF stop, then returns `0x8000` with the actual
stop HRESULT. Failure does not confirm shutdown. A partially written old response
cannot be followed by a Stop receipt on a truncated stream: transport fails closed.
The parent discards old publications when initiating revoke, validates the whole
matching Stop receipt and separately confirms process exit. Exit code and local
enqueue/write completion are not remote receipt proof.

## Lifetime and supervision

Separate pipe reader/writer threads own bounded queues and buffers. The COM MTA
actor services events every 8 ms outside OS calls. Configure must arrive within
five seconds. After successful setup, valid Frame/Poll commands must arrive within
500 ms; a 50 ms parent polling policy is expected. The first keepalive window
begins after setup, but the original overall grant deadline never changes. Later
windows use command receipt time, not completion of slow operations. A responsive
parent pipe is not independent proof of live capture/pair/document authority.

EOF, lease loss, expiry, malformed data or encoder fault fences delivery. NO_WAIT
event reads do not preempt transform/COM/driver calls or in-progress pipe writes.
The parent must supply a creation-time job, original process witness, independent
call/authority watchdog and late-publication fences. Those are not integrated here.
Detached pipe threads retire with process exit; no unbounded join is performed.
The actor allows 500 ms for the final writer, but OS teardown/destruction can still
stall and require the external supervisor. No build, command execution, encoder
activation, media operation or test ran for this checkpoint.
