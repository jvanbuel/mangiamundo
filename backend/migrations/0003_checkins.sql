-- Every check-in is its own row, so returning to a place counts again.
CREATE TABLE checkins (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id   TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    place_id  TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    at        INTEGER NOT NULL,
    -- 1 when the check-in was made within walking distance of the place.
    verified  INTEGER NOT NULL DEFAULT 0,
    -- A signature dish id from data/dishes.json, when the user logged what they ate.
    dish      TEXT
);
CREATE INDEX checkins_user_place ON checkins(user_id, place_id, at);

INSERT INTO checkins (user_id, place_id, at, verified)
SELECT user_id, place_id, visited_at, verified FROM visits;

DROP TABLE visits;

-- A code the user writes down to restore their passport on another device.
ALTER TABLE users ADD COLUMN recovery_code TEXT;
CREATE UNIQUE INDEX users_recovery_code ON users(recovery_code);
