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
the area. Each has the text of its reminder too (`reminder`); the mobile apps no longer
read it, they schedule what `/api/reminders` plans.

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

The iOS and Android apps only schedule: with reminders turned on they ask for the plan
of the current area in the device's time zone when the app starts, when the area
changes and when reminders are turned on, and schedule one local notification per item
at `hour` on `local_date`, local time, with the server's `title` and `body`. The `key`
is the notification's identifier, so scheduling again replaces instead of duplicating,
and what is no longer in the plan is cancelled. The `url` travels with the notification
and decides what a tap opens (`?country=JP` the country, `?tab=badges` the badges tab).
Only the soonest 48 are scheduled, as iOS keeps at most 64 pending notifications per
app, and items whose moment has passed are left out. When the server can't be reached
what was scheduled last stays. No rules or texts live in the apps; a change to `due`
reaches them with the next plan they fetch.

The web app uses Web Push: the server generates a VAPID key on first start (or reads
`VAPID_PRIVATE_KEY`), and every `PUSH_INTERVAL_SECS` (default 600, 0 turns it off)
sends due reminders from 10:00 in each subscriber's time zone. It also says when food
from a country the subscriber hasn't eaten yet appears in their area. Set
`VAPID_SUBJECT` to a `mailto:` or `https:` contact for push services.

Browsers only allow push on HTTPS (or localhost), and iPhones only for apps added to
the home screen.
