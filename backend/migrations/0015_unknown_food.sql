-- Named restaurants in the import box that OpenStreetMap gives no `cuisine` at all. Kept
-- apart from `osm_places`: nobody can play them until a user says what food they serve.
-- Replaced at each import.
CREATE TABLE unknown_places (
    id      TEXT PRIMARY KEY,
    name    TEXT NOT NULL,
    lat     REAL NOT NULL,
    lng     REAL NOT NULL,
    address TEXT
);
CREATE INDEX unknown_places_lat ON unknown_places(lat);

-- "It serves no particular country's food" answers. The place is hidden from whoever
-- answered, and from everyone after two. No foreign key: unknown places come and go with
-- each import.
CREATE TABLE no_cuisine_reports (
    place_id    TEXT NOT NULL,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reported_at INTEGER NOT NULL,
    PRIMARY KEY (place_id, user_id)
);

-- For a pending place made from a suggestion: the unknown OpenStreetMap place it is about.
ALTER TABLE places ADD COLUMN suggested_for TEXT;
CREATE INDEX places_suggested_for ON places(suggested_for);
