-- Rewards for restaurants users add. A row is written when the place is added and outlives
-- the pending `places` row, which the import swaps for its OpenStreetMap twin.
CREATE TABLE additions (
    -- The pending place's `user-…` id.
    place_id     TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    country      TEXT NOT NULL,
    -- The city (or, without one, the postal code) the place lies in, when one was cached.
    area_id      TEXT,
    area_name    TEXT,
    -- Decided when the place was added: was its country, or its whole continent, new there?
    -- Lowered at confirmation when an earlier addition in the area holds the same bonus.
    bonus        TEXT NOT NULL CHECK (bonus IN ('none', 'country', 'continent')),
    submitted_at INTEGER NOT NULL,
    -- Set when the import matched it with an OpenStreetMap place.
    confirmed_at INTEGER,
    osm_place_id TEXT,
    -- Earns XP: confirmed, and nobody added the same place before.
    rewarded     INTEGER NOT NULL DEFAULT 0,
    -- The user was told about the confirmation.
    seen         INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX additions_user ON additions(user_id, submitted_at);
CREATE INDEX additions_area ON additions(area_id, country);
CREATE INDEX additions_osm_place ON additions(osm_place_id);

-- Places added before rewards existed still earn the base reward.
INSERT INTO additions (place_id, user_id, name, country, bonus, submitted_at)
SELECT p.id, p.added_by, p.name, pc.country, 'none', p.added_at
FROM places p JOIN place_countries pc ON pc.place_id = p.id
WHERE p.added_by IS NOT NULL
GROUP BY p.id;
