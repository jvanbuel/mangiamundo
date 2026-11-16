-- OSM opening_hours syntax; Google hours are converted to it.
ALTER TABLE places ADD COLUMN opening_hours TEXT;

-- Cached areas predate opening hours, so fetch them again on next use.
UPDATE areas SET fetched_at = 0;
