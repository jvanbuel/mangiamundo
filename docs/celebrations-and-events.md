# Celebrations, events and reminders

## Data files

- `backend/data/celebrations.json`: national days and food festivals per country.
  Fixed dates use `"date": "MM-DD"`; moving ones list `"dates"` per year (filled in
  for 2026–2028, so extend it before 2029). Moon-sighting dates are marked
  `approximate`.
- `backend/data/events.json`: time-limited challenges ("Japan month"): countries, a
  date range and how many different places to visit. Only check-ins made during the
  event count; earned event badges stay.

`GET /api/celebrations` lists what is coming up for countries that have places in
the area, including the reminder text.

## Time zones

Days are the user's local days: clients send their time zone as `tz` (IANA name, e.g.
`Asia/Tokyo`) to discover, celebrations and check-ins, and each check-in keeps its
local date, so it counts the same wherever the user is later. A missing or unknown
`tz` means UTC.

## Reminders

The native apps schedule local notifications themselves.

The web app uses Web Push: the server generates a VAPID key on first start (or reads
`VAPID_PRIVATE_KEY`), and every `PUSH_INTERVAL_SECS` (default 600, 0 turns it off)
sends due reminders from 10:00 in each subscriber's time zone. It also says when food
from a country the subscriber hasn't eaten yet appears in their area. Set
`VAPID_SUBJECT` to a `mailto:` or `https:` contact for push services.

Browsers only allow push on HTTPS (or localhost), and iPhones only for apps added to
the home screen.
