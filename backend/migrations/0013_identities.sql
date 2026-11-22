-- Ways to sign in to an account besides its sessions and codes: Sign in with Apple, Google,
-- or an email address. The subject is the provider's user id, or the email in lower case.
CREATE TABLE identities (
    provider   TEXT NOT NULL,
    subject    TEXT NOT NULL,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- The address the provider gave last, to show which account it is.
    email      TEXT,
    created_at INTEGER NOT NULL,
    PRIMARY KEY (provider, subject)
);
CREATE INDEX identities_user ON identities(user_id);

-- The latest one-time code sent to an address, as a hash. Wrong guesses count against it.
CREATE TABLE email_codes (
    email      TEXT PRIMARY KEY,
    hash       TEXT NOT NULL,
    expires_at INTEGER NOT NULL,
    attempts   INTEGER NOT NULL DEFAULT 0
);
