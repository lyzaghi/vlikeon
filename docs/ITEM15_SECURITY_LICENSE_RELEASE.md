# Item 15 — security, licensing and release preparation

Owner transition on 2026-10-23: retain mobile gaps and work on 15 then 16.
`DEFERRED_ITEMS_9_10.md` preserves item-9/10 obligations; no platform is closed
by this transition. Other concurrent source is preserved.

## Source changes

`tools/release_inventory.py` adds a publication-side gate used by
`tools/package_release.py` before writing the package. It checks:

- Root LICENSE/NOTICE and shipped third-party/source declarations are present.
- Each payload path has one component owner, explicit distribution review,
  bounded accompanying notices and an explicit corresponding-source obligation.
- Required accompanying source files exist and match their declared SHA-256.
- Case collisions, links, traversal and common profile/credential paths are
  rejected. A streaming content scan rejects selected credential/private-key
  markers without printing matched content. This is deliberately not a complete
  secret detector or a guarantee of credential absence.
- Placing an arbitrary payload in `licenses/` does not exempt it from attribution.

The stricter declaration requires each component to supply `name`, `revision`,
`license`, `distribution_approved: true`, `artifact_paths`, `notice_paths` and
`corresponding_source_required: true|false`. A true source obligation also needs
`source_archive_path` and `source_archive_sha256`; the archive is part of the
package payload and must be attributed. Existing release inputs must be reviewed
and migrated explicitly; no automatic approval or license rewrite is performed.

These gates check declarations and bytes, not legal compatibility, completeness
of corresponding source, license authenticity, code signatures or security.
An operator must review the actual compiled dependency/asset/source combination.
The updater continues to verify its signed manifest and file hashes; this source
change is a build/publication check, not a new client trust root or signature.
The separately built technical demo ZIP is outside the OPPK package flow and is
not retroactively described as passing this new gate.

`tools/inventory_locked_dependencies.py` creates a deterministic review inventory
from Cargo.lock: identity, version, source, checksum and explicitly pending
license/advisory/artifact-inclusion review. All 327 packages in the current lock
are listed locally in `out/item15-locked-dependency-inventory-v1.json`; this
includes optional/dev/platform packages and is **not** the final binary SBOM.
Lock SHA-256: `e0376abb29991e0d2fcda97dd2e6801a77d1dc9fe9972226169dc13365ab1fd4`.

## Still open — no production approval

1. Exact final browser/core/codec/font/firmware asset provenance, full dependency
   notices and license compatibility review. The restricted/unresolved Angrylion
   profile stays research-only; no source receipt can promote it automatically.
2. Complete corresponding-source archives for the exact adapted distributed
   binaries, build scripts and required notices; a matching hash alone is not
   sufficient content review.
3. Artifact-derived SBOM, current advisory/upstream security review, actual
   containment/authentication/update/sync fault evidence and independent review.
4. Real release identity, signing/notarization/store configuration and controlled
   update hosting with offline threshold root. `security/update-channel.json`
   remains disabled; no signing secrets or trust root were created here.
5. Privacy/dependency API and store/export review for the actual mobile binary,
   production assets/accessibility/localization and the deferred source scope.
6. Documented, operational private vulnerability-reporting route and a support
   policy appropriate to the eventually qualified release. This document does
   not enable GitHub private vulnerability reporting or promise an SLA.

Item 15 remains open. Synthetic tests establish only the specified gate behavior;
the subsequent consolidated verification is recorded in `ITEM16_VERIFICATION.md`.
The 13 release-gate cases and 11 updater package/signature integration cases
passed in assigned frozen batches. They do not replace the pending reviews.

Reproduce the synthetic gate cases with `python -B qa/release/test_release_inventory.py`.
Author a fresh locked-source review inventory with
`python -B tools/inventory_locked_dependencies.py --lock Cargo.lock --output <new-file>`.
The latter command does not run a vulnerability database or license audit.
