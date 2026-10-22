# Native N64 Transfer Pak battery clock source

The gl3/1.10 source candidate now has a separate battery record for each RTC
cartridge, including RTC-only cartridges without RAM. This is source work;
no build, core, clock/device, save-cycle or fault case has been executed.

`save/N64/tpak/<32 lowercase ROM-content MD5 hex digits>-p<1..4>.rtc`
is derived by actual main.c from the same ROM/port identity as GB RAM. MD5 is a
content name, not authentication. The registry accepts only this canonical form
and owns at most four stable RTC pointers, with one owner per physical port.
Duplicate RTC/port/path admission fails before rebinding. Names and staged
records live in fixed-size registry storage; there is no host-file write path.

The exact 32-byte `OMGBRTC1` format is independent of snapshot latch state:

| Offset | Size | Value |
| --- | --- | --- |
| 0 | 8 | ASCII `OMGBRTC1` |
| 8 | 5 | Raw live seconds, minutes, hours, day-low, day-high bytes |
| 13 | 3 | Zero reserved bytes |
| 16 | 8 | Signed 64-bit timestamp, two's-complement little-endian wire |
| 24 | 4 | Zero reserved bytes |
| 28 | 4 | IEEE CRC32 of bytes 0–27, little-endian |

CRC detects malformed/corrupted records and does not authenticate save data.
Any wrong length, magic, reserved byte, checksum, read or close error rejects
admission; only literal absence permits a cold battery clock. A complete record
is staged before registration, without mutating RTC outputs. First power-on
restores battery fields before applying the actual bounded offline-time update;
halted clocks exclude the offline interval. Subsequent power-on retains live
battery state. Command/latch validity and frozen reads are volatile on power-on
and remain snapshot-only state in M64P 1.10.

Actual GB construction binds the sidecar before device power-on or first guest
execution. A rejected clock record discards unstarted RAM without publishing a
default, releases its readonly ROM and records owner admission failure. Normal
snapshot restore invalidates any older retirement receipt. Register updates,
clock reads/synchronization, latch changes and power-on also invalidate it.

Aggregate capture flushes RAM, DD and all live RTCs before acknowledgement.
RTC writes use the existing checked save bridge's complete-image publication.
Retirement prepares a current clock image, then closes RAM, then consumes the
still-current RTC receipt. Failed RTC publication retains RAM and RTC owners;
failed RAM close keeps RTC registered, and further clock use invalidates its
old receipt. Unstarted staged owners can retire without writing defaults.
Outstanding RTC ownership blocks quiescence, cart reuse and callback uninstall.
The stable actual `g_dev.gb_carts` RTC object outlives the guest stack until
retirement is confirmed. Unknown stream-close debt still requires the existing
fail-closed actor retirement path.

Each named file publishes atomically; a multi-file RAM/RTC/DD flush is not a
single atomic rename. A failed aggregate can update some in-memory names, but
cannot acknowledge capture or discard remaining producers. Retry uses retained
live state. The worker's complete saved-tree/checkpoint transaction and actual
failure/rollback behavior must be qualified independently; per-file receipts
are not proof of whole-tree durability or process-crash recovery.

Authored `rtc_storage_cases.cpp` uses actual RTC, registry, main aggregate
helpers, file-storage bridge and store, with a synthetic clock and independent
pinned-zlib CRC wire oracle. RTC-only offline/halt cycles, volatile latches,
staged/no-default startup, signed timestamps, corrupt/read-failed images,
four-port bounds, foreign-thread denial and RAM/RTC publication failure/retry
have source cases. Actual main.c binding, worker save/archive restore, close
ambiguities, process restart/crash, cross-profile/content/port behavior, actual
clock/game/device behavior, subsecond timing and every original release gate
remain unverified. Other cartridge implementations, N64 state/resource/write
closure and Windows/product completion remain open. No package admission,
actor OS write grant or product enablement changed.
