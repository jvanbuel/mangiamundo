-- A public, read-only page of a user's progress at /p/{slug}: the map, level, counts and
-- badges for the area they last shared from. Made by sharing; deleting the row turns the
-- link off.
CREATE TABLE public_profiles (
    user_id    TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    slug       TEXT NOT NULL UNIQUE,
    area_id    TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- A shared check-in at /c/{slug}. It goes when the check-in is removed.
CREATE TABLE shared_visits (
    slug       TEXT PRIMARY KEY,
    visit_id   INTEGER NOT NULL UNIQUE REFERENCES visits(id) ON DELETE CASCADE,
    created_at INTEGER NOT NULL
);
