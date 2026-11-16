-- Small server-wide values, such as the generated VAPID key.
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- Browsers that asked for celebration reminders (Web Push).
CREATE TABLE push_subscriptions (
    endpoint   TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    p256dh     TEXT NOT NULL,
    auth       TEXT NOT NULL,
    lat        REAL NOT NULL,
    lng        REAL NOT NULL,
    radius_km  REAL NOT NULL,
    -- IANA time zone, so reminders arrive in the morning where the user is.
    timezone   TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

-- What was already sent, so each reminder goes out once.
CREATE TABLE push_sent (
    endpoint TEXT NOT NULL REFERENCES push_subscriptions(endpoint) ON DELETE CASCADE,
    key      TEXT NOT NULL,
    sent_at  INTEGER NOT NULL,
    PRIMARY KEY (endpoint, key)
);
