-- A local copy of the OpenStreetMap places in the import box, refreshed weekly.
-- Countries are derived from `cuisine` when read, so they follow catalog changes.
CREATE TABLE osm_places (
    id      TEXT PRIMARY KEY,
    name    TEXT NOT NULL,
    lat     REAL NOT NULL,
    lng     REAL NOT NULL,
    address TEXT,
    url     TEXT,
    cuisine TEXT NOT NULL,
    opening_hours TEXT
);
CREATE INDEX osm_places_lat ON osm_places(lat);

-- "This place has closed" reports. A place is hidden from whoever reported it, and
-- from everyone once enough people have.
CREATE TABLE closed_reports (
    place_id    TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    reported_at INTEGER NOT NULL,
    PRIMARY KEY (place_id, user_id)
);
