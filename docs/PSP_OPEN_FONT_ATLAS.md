# PSP atlas from open fonts

The product atlas recipe no longer uses font files installed on the developer's Windows machine. The pinned upstream atlas script referenced KozGoPro and Arial Unicode; those references did not establish redistribution permission. The new source pipeline uses these immutable inputs:

| Input | Revision | Role |
| --- | --- | --- |
| Noto CJK | `f8d157532fbfaeda587e826d4cd5b21a49186f7c` | Japanese/CJK/Korean glyphs from Noto Sans CJK JP Regular |
| Noto Fonts | `ffebf8c1ee449e544955a7e813c54f9b73848eac` | Noto Sans, Hebrew and Thai Regular glyphs |
| FreeType | `063ed27f481f0bcd3962fd468e1546739d7d285a` | Atlas build tool dependency, matching the PPSSPP parent's gitlink |
| PPSSPP | `a5275067b62433375bbde12921e96329a795dff0` | Generator, atlas format, locale selection and control-symbol source images |

`tools/fetch_ppsspp_atlas_sources.py` materializes the selected font blobs through sparse Git checkouts, checks their immutable Git blob identities and records sizes/SHA-256 values in `security/ppsspp-atlas-source.lock.json`. Both selected Noto notices explicitly state OFL 1.1. The build source retains those full notices and the FreeType tool notice. Other optional FreeType codecs/text-shaping libraries are disabled for these uncompressed font inputs.

`tools/build_ppsspp_atlas.py --prepare-only` authors the adapted generator source, local font inputs, notices and atlas script. It excludes the upstream pre-generated `assets` directory from this generator tree. The `UBUNTU24` atlas key is retained for core compatibility; actual outlines now come from Noto. The recipe explicitly links the generator to its Common/zstd helpers. The copied generator is a build tool; it is not included in the runtime package.

The explicit later build compiles the generator and produces `ppge_atlas.zim` and `ppge_atlas.meta`. Its receipt binds the parent revision, font pins, font payload hashes, generator identity, resulting assets and corresponding source. `tools/build_ppsspp.py` requires that receipt for a product core build, checks the generated asset sizes/hashes, replaces the upstream atlas files in its owned corresponding-source tree and retains the new notices. The runtime packager includes the asset corresponding-source/receipt references in provenance and hashes the selected assets into its signed inventory.

Only source preparation has been performed. The generator has not been compiled or run, and no atlas pixels, glyph coverage, metadata behavior or device layout have been verified. The final consolidated pass must generate and inspect the actual atlas, exercise all selected writing systems and ensure its notices/source offer match the distributed artifact. Exact provenance/notices for the separate upstream PGF replacement font files also remain required.

Primary source repositories: [Noto CJK](https://github.com/notofonts/noto-cjk), [Noto Fonts](https://github.com/notofonts/noto-fonts), [pinned FreeType fork](https://github.com/hrydgard/freetype), [PPSSPP](https://github.com/hrydgard/ppsspp).
