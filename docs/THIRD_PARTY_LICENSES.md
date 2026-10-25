# OmniPlay Third-Party Licenses & Component Provenance Ledger (Item 96)

This document contains the verified inventory, license categorization, copyright notices, and source provenance for all third-party software, emulation cores, codecs, fonts, and firmware distributed with or utilized by the OmniPlay (Vlikeon) platform.

---

## 1. Emulation Cores & Adapters

| Component | Upstream Project | Version / Revision | License | Corresponding Source Obligation | Provenance / Adaptation Notes |
|:---|:---|:---|:---|:---:|:---|
| **mGBA** | [mgba-emu/mgba](https://github.com/mgba-emu/mgba) | 0.10.x | MPL-2.0 | **YES** | Embedded libretro core adapter; clean open source GBA runtime. |
| **Beetle PSX** | [libretro/beetle-psx-libretro](https://github.com/libretro/beetle-psx-libretro) | 0.9.44.1 | GPL-2.0-only | **YES** | PlayStation 1 core; includes integrated MIT PCSX-Redux OpenBIOS. |
| **bsnes** | [bsnes-emu/bsnes](https://github.com/bsnes-emu/bsnes) | v115+ | GPL-3.0-only | **YES** | SNES core; adapted by Vlikeon to remove the 64-byte proprietary IPL ROM array. |
| **SameBoy** | [LIJI32/SameBoy](https://github.com/LIJI32/SameBoy) | 0.16.x | MIT | NO (Attribution) | Game Boy / Game Boy Color core and open boot ROMs. |
| **PCSX-Redux OpenBIOS** | [grumpycoders/pcsx-redux](https://github.com/grumpycoders/pcsx-redux) | git rev | MIT | NO (Attribution) | Clean-room open-source PS1 BIOS replacement; no manufacturer firmware required. |
| **PPSSPP** | [hrydgard/ppsspp](https://github.com/hrydgard/ppsspp) | 1.17.x | GPL-2.0-or-later | **YES** | PSP core; IR interpreter execution mode (no-JIT on iOS/AppStore). |

---

## 2. Codecs, Transport & Media Libraries

| Component | Upstream Project | License | Corresponding Source Obligation | Notes |
|:---|:---|:---|:---:|:---|
| **OpenH264** | Cisco Systems, Inc. | BSD-2-Clause | NO (Attribution) | H.264 video encoding and decoding; Cisco binary distribution exemption applies for MPEG-LA patent royalties. |
| **libvpx** | Google WebM Project | BSD-3-Clause | NO (Attribution) | VP8 / VP9 video encoding and decoding for Remote Play. |
| **Opus** | Xiph.Org Foundation | BSD-3-Clause | NO (Attribution) | Low-latency audio codec for Remote Play streaming. |
| **webrtc-audio-processing** | WebRTC Project | BSD-3-Clause | NO (Attribution) | Acoustic echo cancellation and noise suppression. |

---

## 3. Core Dependencies & Frameworks

| Component | Ecosystem | License | Copyright Holder / Author |
|:---|:---|:---|:---|
| **SQLite** | C Library | Public Domain / CC0 | D. Richard Hipp |
| **rusqlite** | Rust Crate | MIT | John Gallagher |
| **ring** | Rust Crate | MIT / ISC | Brian Smith |
| **tokio** | Rust Crate | MIT | Tokio Contributors |
| **serde** | Rust Crate | MIT / Apache-2.0 | David Tolnay |
| **sha2** | Rust Crate | MIT / Apache-2.0 | RustCrypto Project |
| **chacha20poly1305** | Rust Crate | MIT / Apache-2.0 | RustCrypto Project |

---

## 4. Fonts & Graphical Assets

| Asset | Format | License | Copyright / Source |
|:---|:---|:---|:---|
| **Noto Sans** | TTF / WOFF2 | SIL Open Font License 1.1 | Google LLC |
| **Noto Sans Vietnamese** | TTF / WOFF2 | SIL Open Font License 1.1 | Google LLC |
| **Vlikeon Brand Marks** | SVG | Proprietary / All Rights Reserved | Vlikeon Project |

---

## 5. Firmware & Copyright Disclaimer

OmniPlay does NOT distribute, bundle, or host proprietary manufacturer BIOS files or commercial game assets. 
- For systems requiring BIOS (e.g. PS1), OmniPlay utilizes the clean-room MIT-licensed PCSX-Redux OpenBIOS.
- Users supplying proprietary BIOS images must provide their own legally acquired ROM dumps through the local Firmware Vault.
