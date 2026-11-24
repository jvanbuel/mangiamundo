-- Quests belong to an area, so their XP is kept per area: the same quest can be finished
-- in two areas in a month. Rewards from before this have no area ('') and still count.
CREATE TABLE quest_rewards_per_area (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    area_id TEXT NOT NULL DEFAULT '',
    month   TEXT NOT NULL,
    quest   TEXT NOT NULL,
    xp      INTEGER NOT NULL,
    PRIMARY KEY (user_id, area_id, month, quest)
);
INSERT INTO quest_rewards_per_area (user_id, month, quest, xp)
    SELECT user_id, month, quest, xp FROM quest_rewards;
DROP TABLE quest_rewards;
ALTER TABLE quest_rewards_per_area RENAME TO quest_rewards;
