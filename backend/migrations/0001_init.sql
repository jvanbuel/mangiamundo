CREATE TABLE users (
    id         TEXT PRIMARY KEY,
    token      TEXT NOT NULL UNIQUE,
    created_at INTEGER NOT NULL
);

CREATE TABLE places (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    lat        REAL NOT NULL,
    lng        REAL NOT NULL,
    address    TEXT,
    url        TEXT,
    cuisine    TEXT,
    updated_at INTEGER NOT NULL
);

-- A place can represent more than one country (e.g. "thai;vietnamese").
CREATE TABLE place_countries (
    place_id TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    country  TEXT NOT NULL,
    PRIMARY KEY (place_id, country)
);
CREATE INDEX place_countries_country ON place_countries(country);

-- An area is one cached provider lookup: a grid cell centre plus a radius.
CREATE TABLE areas (
    key        TEXT PRIMARY KEY,
    lat        REAL NOT NULL,
    lng        REAL NOT NULL,
    radius_m   INTEGER NOT NULL,
    provider   TEXT NOT NULL,
    fetched_at INTEGER NOT NULL
);

CREATE TABLE area_places (
    area_key TEXT NOT NULL REFERENCES areas(key) ON DELETE CASCADE,
    place_id TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    PRIMARY KEY (area_key, place_id)
);

CREATE TABLE visits (
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    place_id   TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    visited_at INTEGER NOT NULL,
    -- 1 when the check-in was made within walking distance of the place.
    verified   INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (user_id, place_id)
);
