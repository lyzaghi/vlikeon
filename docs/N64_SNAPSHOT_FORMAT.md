# Selected native N64 snapshot protocol

Source-only candidate: `3.0.0+omniplay.gl3.12edd2c74a51`, still unbuilt,
unqualified, unselected by package admission and disabled in the product.
Earlier gl2 preparations remain separate, immutable 1.9-writing candidates.

The current candidate reads canonical M64P versions 1.0 through 1.10 and emits
1.10 (`0x00010a00`, big-endian header version). Unknown minor/patch layouts
reject. Magic, ROM identity, full native buffer size, queue, TLB, PIF and known
Transfer Pak fields retain their existing admission. This is not yet complete
semantic state validation. Normal SRAM/mempak/EEPROM/flash and GB RAM save-file
formats are unaffected by the snapshot-only extension.

The public fixed-size buffer remains `16788288 + 1024 + 4 + 4096` bytes. The
1.9 body and extra-state fields retain their offsets and widths. Version 1.10
adds four little-endian uint32 Boolean flags immediately after the CP0 and CP2
latch words. Each flag is `latched_valid` for the corresponding GB port 0–3.
The existing per-cartridge `rtc.latch` word now retains the full last command
byte (0–255); it no longer doubles as validity of the frozen readable registers.
An exact zero-to-one command captures the live timer. Other commands retain
the last captured registers until the next such edge. A captured latch remains
readable after a zero command or a live timer write.

The optional GB payload remains 92 bytes. Let P be the offset immediately after
the four variable-length Transfer Pak records (each 44 or 136 bytes). The new
flags start at P + 923: PIF/SI/DP/VI 11 bytes, extra RDRAM registers 280, DD 436,
stop field 4, SP FIFOs 32, flash 144, and CP0/CP2 latches 16. The DD component
uses the pinned 19-register layout; the actual selected source has a compile-time
layout contract. All 16 GB presence combinations must preserve this agreement.
Four flags add 16 bytes within the existing reserved 4096-byte extra block.

All flags are staged and validated before guest mutation or callbacks. They
are applied at the matched-cartridge restore point before PIF post-setup;
mismatching/absent cartridges keep their reset state. One invalid later flag
does not publish any earlier flags. Version 1.2–1.9 imports retain the old
Boolean latch interpretation: `latched_valid = rtc.latch != 0`, with the existing
raw timer/register/time fields preserved. Earlier versions use their existing
reset/default path. Old reserved bytes are ignored during legacy import.

There is no claim that an older core can correctly read 1.10 output. The worker,
package, save-envelope and broker must bind exact core/version/ABI identities
when the candidate is eventually admitted; 1.10 must not be mislabeled as gl2
or 1.9, silently downgraded, or tested using an older candidate. Real full-core
roundtrip, all legacy imports, malformed late-port rejection, retry/rollback,
wrong-core rejection and game/device behavior remain item-16 obligations.

The RTC uses the selected seconds-resolution clock backend. Host clock jumps,
halt/resume, day carry and latch behavior have source cases, not execution
evidence. Subsecond oscillator/device timing, hardware behavior of raw invalid
register bytes and actual RTC battery persistence across sessions remain
qualification work; the battery source contract is in `N64_RTC_BATTERY.md`.
No source manifest or format declaration is a
runtime or compatibility result.
