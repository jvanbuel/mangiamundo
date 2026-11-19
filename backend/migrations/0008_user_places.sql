-- Restaurants users add because OpenStreetMap lacks them. They stay pending, shown only to
-- whoever added them, while an OSM note asks mappers to add the real thing. Their ids
-- start with `user-`.
ALTER TABLE places ADD COLUMN added_by TEXT REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE places ADD COLUMN added_at INTEGER;
-- The OSM note asking for it, once posted.
ALTER TABLE places ADD COLUMN osm_note_id INTEGER;
CREATE INDEX places_added_by ON places(added_by, added_at);
