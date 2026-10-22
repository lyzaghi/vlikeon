# Committed PSP and atlas source projects

The current source-only producers use immutable committed Git objects. No
configuration, compiler, atlas executable, decoder, emulator, browser, device or
test has run. These inventories support later builds and review; they are not
signature, compiler, security, license or distribution approval.

## Native source selection

`ppsspp_native_source.py` replaces native working-tree copying with bounded Git
archives at the locked parent and dependency revisions. Changed, ignored,
untracked or populated-submodule working-tree files do not implicitly enter the
product. Each archive records its identity, member count/bytes, selected paths
and omitted executable/game-image filenames. The original archive is retired
after extracting the selected inputs. Archive SHA-256 and pinned Git commit
identity remain in `committed-source.json`.

Extraction admits regular files/directories with finite path, component, depth,
file-count and byte limits. It rejects escaping paths, Windows device names,
case aliases, unexpected links and special files. Deliberately omitted prebuilt
executable/game-image extensions are recorded, including the parent's Vulkan
library links. This finite extension policy is not a complete content/licensing
classification. Limits apply per source archive; total build disk/RSS quotas and
hostile concurrent filesystem replacement still need separate implementation.

The producer compares every selected dependency path/revision with the immutable
parent's gitlinks. The parent has 24 gitlinks: 13 are selected by the existing
lock, including the old FFmpeg identity whose replacement is recorded separately.
Eleven omitted gitlinks have explicit platform/disabled-feature/test/debugger
reasons. An unknown omission or mismatched selected gitlink stops preparation.
The native profile remains Windows libretro; this does not implement macOS,
Linux, Android or iOS runtime isolation. Zstd selects its license files and `lib`
source required by the parent wrapper, excluding its unrelated CLI test links.

The generated version-source CMake script contains the explicit product version.
The parent Git discovery and Windows batch version generator do not supply a
host-dependent fallback version. This removes that mutable version input; it is
not proof of reproducible compiler output or a hermetic toolchain.

## Prepare and build separation

Native preparation is explicit:

```text
python tools/build_ppsspp.py --profile software --prepare-only
python tools/build_ppsspp.py --profile opengl --prepare-only
```

Projects are `out/native-sources/ppsspp-v3-committed` and
`out/native-sources/ppsspp-gl-v3-committed`. A source receipt inventories the
adapted tree, every retained producer recipe, required adapter hashes, source
locks and committed-source record. Core versions remain `sw3`/`gl3`: these are
source-production changes to the same uncompiled decoder/PCM candidate, not an
invented save-format migration. Earlier source projects stay retained.

An actual build requires `--build-prepared`, the selected profile and explicit
real CMake, compiled modern FFmpeg and generated atlas receipt paths. That mode
admits the current identity and unchanged input inventories before making a
separate build-source copy under `out/native-cores`. It does not rebuild a new
product directly from cache working trees. The original prepared tree remains a
retained build input. `--source-project` selects another unlinked workspace source
project explicitly; it does not disable identity/inventory admission.

The future compiled receipt contains the library hash, actual source inventory,
preparation/provenance identities, retained recipes, CMake binary hash and
configuration command. Native packaging requires this committed-source policy,
exact product version and library/source/provenance identity in addition to its
decoder and lifecycle declarations. These local receipts remain unapproved and
are not a substitute for signed release attestations, compiler/linker/SDK
inventories, independently reproduced binaries or complete transitive notices.

## Atlas inputs and generated outputs

`build_ppsspp_atlas.py --prepare-only` uses the same committed parent/dependency
selection with product assets excluded. FreeType is a separately materialized
committed tree at its existing pin. Named Noto font and notice files come directly
from immutable `revision:path` objects, with locked blob identities, byte bounds
and SHA-256 values. Cache working-tree font bytes and host font directories are
not selected inputs. The atlas script comes from the prepared parent source;
its legacy font key is retained for the core while named Noto outlines replace
the old host-font references.

The font lock now uses schema 2 and raw committed blob bytes. The previous lock
combined a Git blob identity with checkout notice bytes whose line endings had
been converted on Windows. A source materialization guard exposed that mismatch;
the old lock is retained under
`out/authoring-history/ppsspp-atlas-raw-blobs-v1`. The source fetcher now inventories
the bounded immutable blob directly, so a checkout's clean/smudge or newline
conversion does not redefine the notice's product bytes. Font outline pins stay
unchanged. FreeType's separate `subprojects/dlg` gitlink is recorded as omitted,
and `FT_DEBUG_LOGGING` is explicitly undefined in the selected font-tool header.
This fixes the tool's declared dependency selection without claiming that the
font parser or generator is secure or qualified.

The prepared project is `out/native-sources/ppsspp-atlas-committed`. Its source,
font/tool provenance, script, notices, locks and producer recipes are inventoried.
`--build-prepared --cmake <actual-path>` admits that source receipt before copying
source to the separate `out/native-assets/ppsspp-atlas-committed` build project.
Only that explicit mode configures/compiles and runs AtlasTool. No atlas has
actually been generated during this authoring milestone.

The authored generated receipt is compact. Its separately hashed
`source-evidence.json` contains the full actual source inventory after generation
and the preparation receipt identity. `ppsspp_atlas_receipt.py` admits the exact
parent/font pins, committed-source policy, generated status, unchanged retained
source/evidence/preparation and exactly two bounded atlas output hashes. Both
native and Web core builds require this generated receipt. A source-only receipt
or the old working-tree producer cannot stand in for it.

Web prepares its updated recipe under
`out/web-cores/ppsspp-v3-committed-atlas`; its existing committed core/SDK/media
selection and PCM behavior remain at `websw3`. The corresponding-source receipt
retains the shared atlas-admission helper, and a future compiled receipt records
the chosen atlas source-evidence identity. Its in-game file wire format remains
unchanged; opaque state identity and the usual exact configuration rules apply.

## Remaining evidence

All four source preparations completed. Native software and OpenGL inventories
each contain 22,921 source files and eight retained producer recipes; Web contains
22,080 source files and ten recipes; the atlas contains 23,514 source files, ten
recipes and four selected font inputs. Every receipt is still source-only with
verification not run; the atlas explicitly records generated false and configure,
build and generator not started. Historical and partial source projects remain
retained. No core, decoder or atlas binary has been promoted.

Source preparation completion does not prove that an omitted dependency is never
reached by every possible CMake configuration. The final consolidated pass must
cover the fixed native/Web/atlas configurations, generated inputs and all actual
toolchain/link dependencies; source/notice/SBOM completeness; compiler/runtime
compatibility; reproducibility; atlas glyph coverage/rendering/format/provenance;
PGF provenance; decoder/backend/PCM correctness; containment, saves, thread and
device performance. All original Chromium, platform, streaming, synchronization,
security, licensing, distribution and final qualification scope stays open.
