-- Countries a push subscriber's area already had, so only newly available ones are announced.
CREATE TABLE push_known_countries (
    endpoint  TEXT NOT NULL REFERENCES push_subscriptions(endpoint) ON DELETE CASCADE,
    country   TEXT NOT NULL,
    -- The last pass that saw it in the area. A country gone for a few weeks is forgotten, so
    -- one that disappears briefly (a closed report, a reverted edit) is not announced again.
    last_seen INTEGER NOT NULL,
    PRIMARY KEY (endpoint, country)
);

-- The area the known countries were taken from. A different area (or none yet) means the
-- next pass takes them again without announcing anything.
ALTER TABLE push_subscriptions ADD COLUMN known_area TEXT;
