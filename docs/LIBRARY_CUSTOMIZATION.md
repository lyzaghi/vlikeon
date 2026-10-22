# Profile appearance and favorites source

These are authored desktop/mobile/Web source changes. No migration, database,
compiler, browser, stylesheet, accessibility check or test has run.

The Settings form now exposes four fixed accent palettes, cover-card/list library
layouts and comfortable/compact spacing. Changes preview locally; leaving Settings
without saving restores the last saved appearance. Restore-default changes the
preview/form and requires the ordinary Save action to persist. Keyboard focus,
minimum touch targets, motion reduction and the existing high-contrast mode retain
their own controls. High-contrast styling takes precedence over custom palettes.
Responsive list rows keep the game title, system and compatibility state visible.
These palettes/layouts still require the final contrast/reflow/device qualification.

An optional defaulted `appearance` object extends schema-1 `UserSettings` without
changing content or save wire formats. Rust and Web validators accept only known
keys/enumerated values. Missing objects/subfields default to lime/cards/comfortable;
explicit invalid values are rejected by persistence, rather than becoming CSS,
remote resources or executable markup. Earlier saved settings remain readable.
Native/mobile persist through the profile SQLite preferences; Web persists through
its strict IndexedDB transaction adapter. Appearance stays in the current profile
and is not included in encrypted game-save sync or diagnostic exports.

Favorites are separate local library metadata, not ROM identity, runtime settings,
save compatibility or a claim of tested game support. The detail dialog has an
accessible pressed-state toggle. The library collection filter combines favorites,
system and text queries across the whole stored library, with bounded result pages
and totals; it does not merely filter an already downloaded first page. Stars mark
favorite titles in both layouts. Title search and favorite lists do not contact a
catalog service or fetch third-party artwork.

SQLite migration 0008 adds a checked 0/1 favorite column plus favorite/title/UUID
and system/favorite/title/UUID indexes, preserving existing game/save records.
Newly imported/registered games start unselected. An idempotent set operation and
its audit record share a transaction; existing-game checks return not-found for a
removed ID. Existing content import, media upgrades and playtime writes retain the
column. The favorite migration establishes version 8; migration 9 adds the save retirement journal described in `SAVE_MAINTENANCE.md`, and current version 10 adds the history paging index. The new-install updater floor follows that current version.
The broker's existing opened-schema high-water path covers upgraded installations,
and old executables must not open the newer database or bypass rollback floors.

The standalone database keeps the same origin/database name and upgrades IndexedDB
from version 1 to 2. Its version-change transaction backfills favorites to numeric
zero and creates matching compound indexes; numeric keys avoid unsupported boolean
index keys. A blocked upgrade retains the existing close-other-tabs flow. Favorite
updates require strict write-transaction completion. Library manifests omit internal
favorite/index fields, while summaries expose a boolean. Content hashes, save
exports, accounting totals and active-session leases do not change.

Native and Web library cursors now use version 2 and bind the favorite filter as
well as text/system. Previously issued transient cursors are rejected; refreshing
the UI starts a new query. Equality clauses expose the native compound indexes,
but actual query plans, large-library latency, migration interruption, disk-full
behavior and concurrent mutations remain final-pass evidence requirements. No
library throughput, migration recovery or visual/accessibility result is claimed.

The consolidated verification must cover fresh and upgraded profiles, settings
without the new fields, invalid/nested settings, all theme/layout/contrast/text-size
combinations, keyboard/controller/touch navigation, favorite toggling from two
windows, duplicate content imports, game removal, filtered pagination/cursor reuse,
native/mobile schema high-water and rollback, IndexedDB blocked/aborted upgrades,
strict transaction failures and unchanged content/save identities.
