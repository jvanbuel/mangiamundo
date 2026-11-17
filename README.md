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

Deploy: `fly deploy --build-arg GIT_COMMIT=$(git describe --always --dirty)`. The
commit shows at the bottom of the web and mobile apps, next to the server's, to tell
old builds apart.

## Areas: cities and postal codes

A location is a city (every postal code in it, e.g. Leuven covers 3000, 3001, 3010,
3012 and 3018) or a single postal code. With GPS the app picks the city you're in
and offers to narrow it to your postal code.

Areas are OpenStreetMap boundaries (`boundary=administrative` at municipality level,
and `boundary=postal_code`). Nominatim finds them and provides the outline; Overpass
is asked for the restaurants in the outline's bounding box, and only those inside
the outline are kept. So restaurants without an `addr:postcode` tag count too.
Where a country has no postal-code boundaries in OpenStreetMap, only cities are
offered. Areas larger than 1.5° are refused.

## Where the restaurants come from

Set `PLACES_PROVIDER` (see `backend/.env.example` for all settings).

| Provider | Key | Coverage |
| --- | --- | --- |
| `osm` (default) | none | OpenStreetMap `cuisine=*` tags via Overpass (the main server plus two mirrors, tried in order; override with `OVERPASS_URLS`), areas via Nominatim (at most one request per second, per its usage policy). About 110 national cuisines. Quality depends on local mappers. |
| `demo` | none | Made-up restaurants in a few cities, each split into two made-up postal codes. For development and tests. |

The cuisine → country table lives in `backend/src/catalog.rs`. Regional labels
("asian", "mediterranean") and globalised dishes (pizza, kebab, burger) are left
out on purpose.

Results are cached per area (with its outline) and refreshed after `CACHE_TTL_DAYS`.

## Celebrations and limited-time badges

- `backend/data/celebrations.json`: national days and food festivals per country.
  Fixed dates use `"date": "MM-DD"`; moving ones list `"dates"` per year (filled in
  for 2026–2028, so extend it before 2029). Moon-sighting dates are marked
  `approximate`.
- `backend/data/events.json`: time-limited challenges ("Japan month"): countries, a
  date range and how many different places to visit. Only check-ins made during the
  event count; earned event badges stay. Days are the user's local days: clients send
  their time zone as `tz` (IANA name, e.g. `Asia/Tokyo`) to discover, celebrations and
  check-ins, and each check-in keeps its local date, so it counts the same wherever
  the user is later. A missing or unknown `tz` means UTC.
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
- **Completed**: you visited every place of that country in your area.
- **Continent / world badges**: taste every country that is available in your city.
- **XP**: 10 per check-in, 50 per new country. Level *n* starts at 100·(n−1)² XP.
- A check-in made within 250 m of the place is marked as verified.

Accounts are anonymous: each device asks `POST /api/session` for a token once and
sends it as `Authorization: Bearer <token>`.

## API

JSON field names are snake_case on the wire (the TypeScript types spell them
raw, Kotlin maps them with `@SerialName`, Swift with `convertFromSnakeCase`).
Optional fields are always sent, as `null` when empty. Timestamps are Unix seconds, dates are
`YYYY-MM-DD`. Every error, including unknown `/api` paths, is
`{ "error": "message", "code": "..." }` with a 4xx or 5xx status. The message is
for people and may change; clients that react to a specific case check `code`:

| `code` | Status | |
| --- | --- | --- |
| `bad_request` | 400, 413, 415, 422 | Invalid or missing parameters, a malformed or oversized (over 2 MiB) body, or a body without `Content-Type: application/json` |
| `unauthorized` | 401 | Missing or unknown session token |
| `not_found` | 404 | Unknown place, area or `/api` path |
| `method_not_allowed` | 405 | The path exists, but not with this method |
| `upstream` | 502 | The places or areas provider failed |
| `internal` | 500 | Anything else, such as a database error |

Every route, parameter and response is described in the OpenAPI 3.1 document,
served at `GET /api/openapi.json` and checked in as
[`contract/openapi.json`](contract/openapi.json). It is generated from the handlers
with [utoipa](https://github.com/juhaku/utoipa), and an API change shows up in that
file's diff. The web app's types (`web/src/lib/api-types.ts`) are generated from it:
run `npm run api-types` in `web/` after changing the API.

## Data credits

Typeface: Nunito by Vernon Adams and contributors, SIL Open Font License
(`mobile/shared/Nunito-OFL.txt`). Country borders: Natural Earth via `world-atlas`. Country metadata:
`world-countries`. Restaurant data: © OpenStreetMap contributors (ODbL) when using
the `osm` provider.

## Look and feel

The app follows the system colour scheme and has an Auto / Light / Dark switch on
the map. All colours, including the map's, are CSS custom properties in
`web/src/app.css`. The app icon is `web/static/icon.svg`.
