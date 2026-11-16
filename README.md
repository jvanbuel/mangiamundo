<h1>
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/logo-dark.svg">
    <img src="docs/logo.svg" alt="Mangiamundo: eat the world, start with your city" width="440">
  </picture>
</h1>

Eat your way around the world without leaving your city. Mangiamundo shows a world
map (flat or as a 3D globe) where every country whose cuisine you can eat nearby
lights up. Check in at restaurants to taste countries, complete them, finish
continents and earn badges.

- `backend/` — Rust (axum + SQLite) API: place lookup, caching, accounts, game logic.
- `web/` — SvelteKit single-page app with a MapLibre map.
- `mobile/` — native Android (Kotlin, Jetpack Compose) and iOS (Swift, SwiftUI) apps;
  see `mobile/README.md`.

## Run it

```sh
# 1. API on http://localhost:8080 (demo = offline made-up restaurants)
cd backend
PLACES_PROVIDER=demo cargo run

# 2. Web app on http://localhost:5173, proxying /api to the backend
cd web
npm install
npm run dev
```

For a single deployable: `npm run build` in `web/`, then `cargo run --release` in
`backend/` serves the app and the API together on port 8080.

Tests: `cargo test` in `backend/`, `npm run check` in `web/`.

## Where the restaurants come from

Set `PLACES_PROVIDER` (see `backend/.env.example` for all settings).

| Provider | Key | Coverage |
| --- | --- | --- |
| `osm` (default) | none | OpenStreetMap `cuisine=*` tags via Overpass (the main server plus two mirrors, tried in order; override with `OVERPASS_URLS`), geocoding via Nominatim. About 110 national cuisines. Quality depends on local mappers. |
| `google` | `GOOGLE_MAPS_API_KEY` | Places API (New) Nearby Search plus the Geocoding API. Only the ~50 cuisines Google has a place type for, at most 20 places per cuisine. One request per cuisine per area refresh, billed by Google. |
| `demo` | none | Deterministic fake data for development and tests. |

The cuisine → country table lives in `backend/src/catalog.rs`. Regional labels
("asian", "mediterranean") and globalised dishes (pizza, kebab, burger) are left
out on purpose.

Results are cached per area: coordinates snap to a ~2 km grid, and an area is
refreshed after `CACHE_TTL_DAYS`. If you go to production with Google data, check
the Maps Platform terms on how long place details may be stored.

## Celebrations and limited-time badges

- `backend/data/celebrations.json`: national days and food festivals per country.
  Fixed dates use `"date": "MM-DD"`; moving ones list `"dates"` per year (filled in
  for 2026–2028, so extend it before 2029). Moon-sighting dates are marked
  `approximate`.
- `backend/data/events.json`: time-limited challenges ("Japan month"): countries, a
  date range and how many different places to visit. Only check-ins made during the
  event count; earned event badges stay.
- `GET /api/celebrations` lists what is coming up for countries that have places in
  the area, including the reminder text.

Reminders: the native apps schedule local notifications themselves. The web app uses
Web Push: the server generates a VAPID key on first start (or reads
`VAPID_PRIVATE_KEY`), and every `PUSH_INTERVAL_SECS` (default 600, 0 turns it off)
sends due reminders from 10:00 in each subscriber's time zone. Set `VAPID_SUBJECT`
to a `mailto:` or `https:` contact for push services. Browsers only allow push on
HTTPS (or localhost), and iPhones only for apps added to the home screen.

## How the game works

- **Tasted**: you checked in at one or more places of a country.
- **Completed**: you visited every place of that country within your radius.
- **Continent / world badges**: taste every country that is available in your city.
- **Coming back counts**: every check-in is recorded, at most one per place every 12
  hours. Five check-ins at one place earn the *Regular* badge.
- **Signature dishes**: right after a check-in you can log which of the country's
  signature dishes you had (`backend/data/dishes.json`, three per cuisine). Each
  different dish is worth bonus XP, with badges at 1, 10 and 25 dishes. A dish id is
  the country code plus a slug of its name (`jp-ramen`), so renaming a dish in the file
  orphans the check-ins that logged it.
- **XP**: 10 per check-in, 50 per new country, 25 per new signature dish. Level *n*
  starts at 100·(n−1)² XP.
- A check-in made within 250 m of the place is marked as verified.

## Backups and share cards

- **Recovery code**: every account can show a 16-character code (Crockford base32, so
  O/0 and I/1 mix-ups are forgiven). Entering it on another device moves that device
  onto the passport; whatever the device had collected on its own is merged in. On the
  web the code also comes as a QR code that opens `/?restore=CODE`. "New code" retires
  the old one.
- **Share card**: `GET /api/share/card.png` draws a 1080×1350 PNG on the server (resvg,
  Nunito from `backend/assets/fonts`, outlines from `mobile/shared/world.json`), so the
  web app and both native apps share one design and only have to show and share it.

Accounts are anonymous: each device asks `POST /api/session` for a token once and
sends it as `Authorization: Bearer <token>`. A recovery code moves the account to
another device.

## API

| Method | Path | |
| --- | --- | --- |
| `POST` | `/api/session` | Create an anonymous account, returns `{ token }` |
| `GET` | `/api/session/recovery` | This account's recovery code, `{ code }` |
| `POST` | `/api/session/recovery` | Replace the recovery code |
| `POST` | `/api/session/restore` | `{ code }`: returns `{ token, merged }` for that account and merges the calling account into it |
| `GET` | `/api/countries` | Country, continent and signature dish catalog |
| `GET` | `/api/discover?lat&lng&radius_km` | Countries available here, progress, badges, profile |
| `GET` | `/api/places?lat&lng&radius_km&country` | Places, nearest first, with visit counts and the latest check-in |
| `POST` | `/api/visits/{place_id}` | Check in (again); optional body `{ lat, lng, dish }`. 409 within 12 hours of the last one |
| `PUT` | `/api/visits/{place_id}` | First check-in only (for older clients) |
| `DELETE` | `/api/visits/{place_id}` | Remove every check-in at a place |
| `PATCH` | `/api/checkins/{id}` | `{ dish }`: log or clear (`null`) the dish eaten; `PUT` works too |
| `DELETE` | `/api/checkins/{id}` | Undo one check-in |
| `GET` | `/api/share/card.png?lat&lng&radius_km&place&theme` | Share card PNG, `theme` is `light` or `dark` |
| `GET` | `/api/celebrations?lat&lng&radius_km&days` | Upcoming celebrations with food nearby |
| `GET` | `/api/push/key` | VAPID public key for Web Push |
| `PUT` | `/api/push/subscription` | Subscribe a browser: PushSubscription JSON plus `lat`, `lng`, `radius_km`, `timezone` |
| `DELETE` | `/api/push/subscription` | Unsubscribe; body `{ endpoint }` |
| `GET` | `/api/geocode?q` | Search for a location |
| `GET` | `/api/geocode/reverse?lat&lng` | Name of the place at a position |

## Data credits

Typeface: Nunito by Vernon Adams and contributors, SIL Open Font License
(`mobile/shared/Nunito-OFL.txt`). Country borders: Natural Earth via `world-atlas`. Country metadata:
`world-countries`. Restaurant data: © OpenStreetMap contributors (ODbL) when using
the `osm` provider.

## Look and feel

The app follows the system colour scheme and has an Auto / Light / Dark switch on
the map. All colours, including the map's, are CSS custom properties in
`web/src/app.css`. The app icon is `web/static/icon.svg`.
