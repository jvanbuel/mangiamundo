-- Areas used to be a grid cell plus a radius; now they are a city or a postal code with
-- its outline. The old entries are only a cache, so they go. Places and visits stay.
DROP TABLE area_places;
DROP TABLE areas;

CREATE TABLE areas (
    -- Provider-prefixed, e.g. osm-relation-196311.
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    -- city or postal_code
    kind       TEXT NOT NULL,
    detail     TEXT,
    lat        REAL NOT NULL,
    lng        REAL NOT NULL,
    -- GeoJSON MultiPolygon coordinates, so a refresh doesn't need the directory again.
    shape      TEXT NOT NULL,
    provider   TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);

CREATE TABLE area_places (
    area_id  TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
    place_id TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    PRIMARY KEY (area_id, place_id)
);

-- Reminders follow an area instead of a point and radius. Existing subscriptions get
-- theirs when the app next opens (it re-subscribes on every refresh); until then the
-- push loop skips them.
ALTER TABLE push_subscriptions ADD COLUMN area_id TEXT;
ALTER TABLE push_subscriptions DROP COLUMN lat;
ALTER TABLE push_subscriptions DROP COLUMN lng;
ALTER TABLE push_subscriptions DROP COLUMN radius_km;
