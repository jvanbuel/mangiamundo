-- Countries a subscriber's area already had, so only new ones are announced. Rows are
-- never removed when a country leaves the area, so one that comes back is not news.
CREATE TABLE push_known_countries (
    endpoint TEXT NOT NULL REFERENCES push_subscriptions(endpoint) ON DELETE CASCADE,
    country  TEXT NOT NULL,
    PRIMARY KEY (endpoint, country)
);

-- The area the known countries are from. A new subscription or a new area starts from
-- what is there, so subscribing or moving sends nothing.
ALTER TABLE push_subscriptions ADD COLUMN known_area_id TEXT;
