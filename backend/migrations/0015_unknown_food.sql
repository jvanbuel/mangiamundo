-- Restaurants OpenStreetMap has without a `cuisine`. They are kept like any other place,
-- but with no country they count for nothing until players say which country's food
-- they serve. The local copy keeps them too, so `cuisine` may now be empty there.
CREATE TABLE osm_places_new (
    id      TEXT PRIMARY KEY,
    name    TEXT NOT NULL,
    lat     REAL NOT NULL,
    lng     REAL NOT NULL,
    address TEXT,
    url     TEXT,
    cuisine TEXT,
    opening_hours TEXT
);
INSERT INTO osm_places_new SELECT id, name, lat, lng, address, url, cuisine, opening_hours FROM osm_places;
DROP TABLE osm_places;
ALTER TABLE osm_places_new RENAME TO osm_places;
CREATE INDEX osm_places_lat ON osm_places(lat);

-- What players say a place without a cuisine serves. One answer per player and place, final.
CREATE TABLE cuisine_suggestions (
    place_id     TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- ISO alpha-2, or NULL for "no particular country's food".
    country      TEXT,
    suggested_at INTEGER NOT NULL,
    PRIMARY KEY (place_id, user_id)
);
CREATE INDEX cuisine_suggestions_user ON cuisine_suggestions(user_id, suggested_at);

-- A country only one player suggested counts only for them; NULL means for everyone.
ALTER TABLE place_countries ADD COLUMN suggested_by TEXT REFERENCES users(id) ON DELETE CASCADE;

-- Fetch every area and the local copy again, so the places without a cuisine show up.
UPDATE areas SET fetched_at = 0;
UPDATE settings SET value = '0' WHERE key = 'osm_import_at';
