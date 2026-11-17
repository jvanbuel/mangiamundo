-- The calendar date of a check-in where it was made (YYYY-MM-DD), so event windows
-- ("Japan month") judge each visit by the user's local day, whatever time zone a later
-- request comes from. Visits from before this migration were counted in UTC; keep that.
ALTER TABLE visits ADD COLUMN local_date TEXT NOT NULL DEFAULT '';
UPDATE visits SET local_date = date(visited_at, 'unixepoch');
