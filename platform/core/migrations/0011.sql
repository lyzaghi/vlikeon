BEGIN IMMEDIATE;
CREATE TABLE save_origins (
    save_id TEXT PRIMARY KEY REFERENCES saves(id) ON DELETE CASCADE,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('captured','authenticated_remote','imported','legacy_projection')),
    canonical_metadata_json TEXT,
    canonical_hash TEXT CHECK(canonical_hash IS NULL OR length(canonical_hash)=64),
    canonical_parent_id TEXT,
    imported_from_json TEXT,
    imported_from_hash TEXT CHECK(imported_from_hash IS NULL OR length(imported_from_hash)=64),
    CHECK((source_kind='legacy_projection' AND canonical_metadata_json IS NULL AND canonical_hash IS NULL AND canonical_parent_id IS NULL)
       OR (source_kind!='legacy_projection' AND canonical_metadata_json IS NOT NULL AND canonical_hash IS NOT NULL)),
    CHECK((imported_from_json IS NULL)=(imported_from_hash IS NULL)),
    CHECK((source_kind='imported')=(imported_from_json IS NOT NULL))
) STRICT;
CREATE INDEX save_origin_parent ON save_origins(canonical_parent_id);
-- Existing local projections cannot prove their former remote game/generation/
-- parent values. Preserve them without inventing a canonical capture record.
INSERT INTO save_origins(save_id,source_kind) SELECT id,'legacy_projection' FROM saves;
-- A previously opened older writer can finish during an upgrade. It cannot
-- fabricate origin evidence, but its new row must still have an honest marker.
CREATE TRIGGER save_origin_legacy_fallback AFTER INSERT ON saves BEGIN
    INSERT INTO save_origins(save_id,source_kind) VALUES(NEW.id,'legacy_projection');
END;
PRAGMA user_version=11;
COMMIT;
