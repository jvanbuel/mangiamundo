# Celebrations, events and reminders

## Data files

- `backend/data/celebrations.json`: national days and food festivals per country.
  Fixed dates use `"date": "MM-DD"`; moving ones list `"dates"` per year (filled in
  for 2026–2028; a test fails once next year is missing). Moon-sighting dates are marked
  `approximate`.
- `backend/data/events.json`: time-limited challenges ("Japan month"): countries, a
  date range and how many different places to visit. Only check-ins made during the
  event count; earned event badges stay.

`GET /api/celebrations` lists what is coming up for countries that have places in
the area, including the reminder text.

## Monthly quests

Every area has its own quests each month (`monthly_quests` in `backend/src/game.rs`):
"two check-ins" and up to two more, picked by area and month from what the area has and
what the player had done before the month began. They are done in the area: only that
month's check-ins at the area's places count, so the same quest can be finished, and
paid, in two areas. What makes a country new is the player's whole history, anywhere.

The XP of a finished quest is kept in `quest_rewards`, per user, area, month and quest.
It is booked by the check-in that finishes the quest (`settle_quests` in
`backend/src/progress.rs`), in every cached area the place is in, whether or not the
check-in names an `area`; a postal code and the city around it both pay. The check-in's
`profile` already has it. Removing a check-in takes back the quests of its month that are
no longer finished. Nothing else books quest XP: `GET /api/discover`, the reminders plan
and the push loop only read.

## Time zones

Days are the user's local days: clients send their time zone as `tz` (IANA name, e.g.
`Asia/Tokyo`) to discover, celebrations and check-ins, and each check-in keeps its
local date, so it counts the same wherever the user is later. A missing or unknown
`tz` means UTC.

## Reminders

Which reminders are due on a day is decided in one place, `due` in
`backend/src/push.rs`: celebrations and events on their first day, a "last days" nudge
three days before an event ends for who hasn't earned its badge, and the month's quests
on the 1st.

`GET /api/reminders?area=<id>&tz=<iana>&days=<n>` lists what becomes due from today up
to `days` days ahead (60 by default, at most 366), for apps that schedule local
notifications themselves. Each item has the `key` to schedule it under, its
`local_date`, the `hour` it is sent from (10), and the notification's `title`, `body`,
`url` and `tag`. It works without a token, as for someone with no visits. The plan
follows the user's visits, so apps ask again after a check-in. Asking sends and records
nothing.

The web app uses Web Push: the server generates a VAPID key on first start (or reads
`VAPID_PRIVATE_KEY`), and every `PUSH_INTERVAL_SECS` (default 600, 0 turns it off)
sends due reminders from 10:00 in each subscriber's time zone. It also says when food
from a country the subscriber hasn't eaten yet appears in their area. Set
`VAPID_SUBJECT` to a `mailto:` or `https:` contact for push services.

Browsers only allow push on HTTPS (or localhost), and iPhones only for apps added to
the home screen.
