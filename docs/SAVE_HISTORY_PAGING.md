# Complete local history paging source

Game details now use a versioned `save_page` interface instead of the legacy
500-record list. The view keeps fifty records in the DOM at a time, with previous,
next and refresh controls, a real total count, slot/kind/label/compatibility fields,
per-record export and compare-and-swap promotion. Download links remain in the
separate persistent detail-dialog dock when the history page changes. These are
authored sources; no query, migration, UI, compiler or test has run.

The backend accepts one to one hundred records and caps serialized metadata at
roughly 256 KiB per page before framing overhead. It never includes payload bytes
or a browser Blob in this response. Every entry carries its actual current head
ID for that game/kind/slot, so the view does not need an unbounded all-heads query.
Promotion still verifies payload integrity and the expected current head before
changing which progress is current. Prior records are retained.

Version-1 canonical base64url cursors bind the game UUID and descending
generation/creation-time/UUID position. SQL uses bound values and checks that the
stored metadata matches the row's actual identity/order fields. IndexedDB uses its
existing compound history index and the equivalent exclusive upper bound. Counts,
entries and head IDs come from one read transaction per page. Pages are independent
observations: concurrent saves/removals/promotions may change totals or results,
and Refresh starts a new traversal. This is not a frozen backup snapshot.

SQLite migration 0010 adds the matching game/history ordering index. The current
native profile schema and fresh-install updater floor are 10; opened-schema
high-water and rollback checks retain their existing role. Standalone IndexedDB
remains version 3 because its history index already exists. Legacy `save_list`
continues to serve older internal consumers and is not evidence of full paging.

Native metadata validation now matches Web string/numeric bounds, including
bounded core version/platform/state-format strings and interoperable safe integer
generation/timestamps. Save admission checks the metadata-envelope bound before
writing a new payload, preventing an unreadable oversized metadata record from
being admitted. Continuing a restored parent keeps that parent's save slot rather
than submitting the new generation against slot zero. Exporting an independent
live copy still intentionally uses a fresh parentless record.

The final consolidated pass must include zero/single/many pages, more than five
hundred records, byte-budget pagination, equal timestamps/generations across slots,
cursor tampering/wrong-game/canonical encoding, transaction/read failures, stale
head promotion, concurrent insertion/removal, labels/large text, keyboard/controller/
touch navigation and persistent download links. Native migration/query plans and
real large-library latency still need evidence. Bulk backup/restore, fuller slot
selection UX, storage recovery and the remaining product scope stay required.
