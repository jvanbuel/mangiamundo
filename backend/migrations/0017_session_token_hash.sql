-- A session token is the account, so only its SHA-256 is kept, like recovery and link
-- codes. SQLite can't hash: the tokens that are here in plain text are marked, and the
-- server hashes the marked ones when it starts (`account::hash_plain_tokens`). Devices
-- keep the token they have.
ALTER TABLE sessions RENAME COLUMN token TO token_hash;
UPDATE sessions SET token_hash = 'plain:' || token_hash;
