# OmniPlay Corresponding Source Distribution (Item 97)

This document details the corresponding source code policy, archive structure, and verification procedures required under GPL-2.0, GPL-3.0, and MPL-2.0 licenses for components distributed in binary form with OmniPlay.

---

## 1. Compliance Obligations

Under Section 3 of GPL-2.0 and Section 6 of GPL-3.0, distributors of binary executables must provide the complete corresponding machine-readable source code, including:
1. Exact source code files used to generate the binary.
2. Any adaptation, bugfix, or sandboxing patches applied by Vlikeon.
3. Build scripts, compilation flags, and toolchain versions.
4. Copyright notices and license grants.

---

## 2. Core Adaptations & Patches

### 2.1 bsnes (GPL-3.0-only)
- **Adaptation**: Upstream bsnes contains an embedded 64-byte array representing Nintendo's proprietary IPL boot ROM. 
- **Vlikeon Patch**: `bsnes-remove-ipl-array.patch` removes this embedded ROM array and replaces it with clean-room memory initialization, preventing distribution of non-free byte sequences.
- **Corresponding Source**: Full bsnes source tree including this patch is archived in `omniplay-corresponding-source-v1.tar.gz`.

### 2.2 Beetle PSX (GPL-2.0-only)
- **Adaptation**: Configured to run with the integrated clean-room PCSX-Redux OpenBIOS by default.
- **Corresponding Source**: Beetle PSX libretro source tree and build scripts.

### 2.3 mGBA (MPL-2.0)
- **Adaptation**: Static sandboxed compilation without external network capabilities.
- **Corresponding Source**: Complete mGBA source and libretro wrapper.

---

## 3. Automated Archive Generation

The corresponding source archive is generated automatically by:
```bash
py tools/archive_corresponding_source.py
```

Output:
- Archive: `dist/source/omniplay-corresponding-source-v1.tar.gz`
- Metadata: `dist/source/omniplay-corresponding-source-v1.json` (contains SHA-256 and component inventory)
