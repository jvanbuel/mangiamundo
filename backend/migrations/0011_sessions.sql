-- One session per device, so a device can be signed out on its own and an account can be
-- used on several devices. `users.token` was the only session; it moves here and the
-- column keeps the user's id, only because SQLite can't drop a UNIQUE column.
CREATE TABLE sessions (
    id           TEXT PRIMARY KEY,
    token        TEXT NOT NULL UNIQUE,
    user_id      TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- What the client called itself ("iPhone", "Android", "Web"), shown in the device list.
    device       TEXT,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER NOT NULL
);
CREATE INDEX sessions_user ON sessions(user_id);

INSERT INTO sessions (id, token, user_id, device, created_at, last_used_at)
SELECT lower(hex(randomblob(16))), token, id, NULL, created_at, created_at FROM users;
UPDATE users SET token = id;

-- SHA-256 of the recovery code, which restores the account on any device. Only the
-- latest code works.
ALTER TABLE users ADD COLUMN recovery_hash TEXT;
ALTER TABLE users ADD COLUMN recovery_created_at INTEGER;
CREATE UNIQUE INDEX users_recovery_hash ON users(recovery_hash);

-- Short-lived, single-use codes that sign another device in to the same account.
CREATE TABLE link_codes (
    hash       TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at INTEGER NOT NULL
);
