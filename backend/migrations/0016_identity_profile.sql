-- The name and photo Google shares with a sign-in, shown only to the user, in the app's
-- header. Apple and email sign-ins have none.
ALTER TABLE identities ADD COLUMN name TEXT;
ALTER TABLE identities ADD COLUMN picture TEXT;
