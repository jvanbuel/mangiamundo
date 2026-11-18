-- Visits become a history: one row per check-in instead of one per place. A place still
-- counts once for the game, however often it is visited. At most one visit per place per
-- local day, so a double tap doesn't add two.
CREATE TABLE visit_history (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    place_id   TEXT NOT NULL REFERENCES places(id) ON DELETE CASCADE,
    visited_at INTEGER NOT NULL,
    -- 1 when the check-in was made within walking distance of the place.
    verified   INTEGER NOT NULL DEFAULT 0,
    -- The calendar date of the check-in where it was made (YYYY-MM-DD).
    local_date TEXT NOT NULL,
    UNIQUE (user_id, place_id, local_date)
);

-- Existing visits become the first entry of each history.
INSERT INTO visit_history (user_id, place_id, visited_at, verified, local_date)
SELECT user_id, place_id, visited_at, verified, local_date FROM visits ORDER BY visited_at;

DROP TABLE visits;
ALTER TABLE visit_history RENAME TO visits;
