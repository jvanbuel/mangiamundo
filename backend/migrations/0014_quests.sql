-- XP from finished monthly quests, kept once earned. `month` is e.g. 2026-11.
CREATE TABLE quest_rewards (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    month   TEXT NOT NULL,
    quest   TEXT NOT NULL,
    xp      INTEGER NOT NULL,
    PRIMARY KEY (user_id, month, quest)
);
