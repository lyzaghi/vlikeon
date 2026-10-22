# Licensing boundaries

The project owner selected Apache License 2.0 for original project material on
2026-10-22. Original source and documentation in this repository are licensed
under the root `LICENSE` unless an existing file or component notice states
otherwise. Copyright remains with the respective authors.

## Preserved exceptions

- `browser/upstream-snapshots/` and `browser/android-snapshots/` contain
  Chromium-derived reference files. Preserve their copyright headers and
  the BSD-style notice in `licenses/Chromium-LICENSE`.
- Chromium patches and derivatives retain applicable upstream obligations.
- `engines/native/adapters/LICENSE.libco` and
  `engines/web/adapters/LICENSE.libco` retain their ISC-style grants.
- `engines/web/adapters/LICENSE.threaded` and
  `engines/web/adapters/LICENSE.webgl2` retain their MIT grants.
- Referenced upstream checkouts retain all per-file and component licenses.
  See `security/dependencies.json` and `security/retro-provenance-review.json`.

This is a starting inventory, not a completed audit of every file, generated
derivative or binary. Preserve upstream notices wherever they occur.

## Distribution is a separate decision

The root license does not approve combined Chromium, emulator, firmware, font
or codec distribution. Proposed cores include GPL/LGPL obligations and unresolved
provenance. Selected candidates remain unapproved for product distribution.
No claim is made that every combination is license-compatible or that all
corresponding-source obligations have been satisfied.

Commercial games, manufacturer BIOS and private user content are not supplied
or licensed by this repository. Upstream replacement firmware and embedded
assets require separate review.

The Vlike and Vlikeon names are identifiers; the source license does not grant
endorsement or a separate trademark license. No OpenAI, Google or GitHub
affiliation or approval is claimed.
