-- Push subscriptions also hold the iOS and Android apps' device tokens, and say which
-- notifications each one wants.
-- `web` (Web Push), `apns`, `apns-sandbox` (development builds) or `fcm`. For an app the
-- endpoint is `<kind>:<device token>` and the Web Push keys are empty.
ALTER TABLE push_subscriptions ADD COLUMN kind TEXT NOT NULL DEFAULT 'web';
-- Celebration and event reminders. The apps schedule those themselves, so off for them.
ALTER TABLE push_subscriptions ADD COLUMN reminders INTEGER NOT NULL DEFAULT 1;
-- News of food not tasted yet becoming available in the area. Opt-in.
ALTER TABLE push_subscriptions ADD COLUMN new_food INTEGER NOT NULL DEFAULT 0;
