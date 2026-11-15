# Mangiamundo

Eat your way around the world without leaving your city. Mangiamundo shows a world
map (flat or as a 3D globe) where every country whose cuisine you can eat nearby
lights up. Check in at restaurants to taste countries, complete them, finish
continents and earn badges.

- `backend/` — Rust (axum + SQLite) API: place lookup, caching, accounts, game logic.
- `web/` — SvelteKit single-page app with a MapLibre map.

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
| `osm` (default) | none | OpenStreetMap `cuisine=*` tags via Overpass, geocoding via Nominatim. About 110 national cuisines. Quality depends on local mappers. |
| `google` | `GOOGLE_MAPS_API_KEY` | Places API (New) Nearby Search plus the Geocoding API. Only the ~50 cuisines Google has a place type for, at most 20 places per cuisine. One request per cuisine per area refresh, billed by Google. |
| `demo` | none | Deterministic fake data for development and tests. |

The cuisine → country table lives in `backend/src/catalog.rs`. Regional labels
("asian", "mediterranean") and globalised dishes (pizza, kebab, burger) are left
out on purpose.

Results are cached per area: coordinates snap to a ~2 km grid, and an area is
refreshed after `CACHE_TTL_DAYS`. If you go to production with Google data, check
the Maps Platform terms on how long place details may be stored.

## How the game works

- **Tasted**: you checked in at one or more places of a country.
- **Completed**: you visited every place of that country within your radius.
- **Continent / world badges**: taste every country that is available in your city.
- **XP**: 10 per check-in, 50 per new country. Level *n* starts at 100·(n−1)² XP.
- A check-in made within 250 m of the place is marked as verified.

Accounts are anonymous: each device asks `POST /api/session` for a token once and
sends it as `Authorization: Bearer <token>`.

## API

| Method | Path | |
| --- | --- | --- |
| `POST` | `/api/session` | Create an anonymous account, returns `{ token }` |
| `GET` | `/api/countries` | Country and continent catalog |
| `GET` | `/api/discover?lat&lng&radius_km` | Countries available here, progress, badges, profile |
| `GET` | `/api/places?lat&lng&radius_km&country` | Places, nearest first |
| `PUT` | `/api/visits/{place_id}` | Check in; optional body `{ lat, lng }` to verify |
| `DELETE` | `/api/visits/{place_id}` | Undo a check-in |
| `GET` | `/api/geocode?q` | Search for a location |
| `GET` | `/api/geocode/reverse?lat&lng` | Name of the place at a position |

## Data credits

Country borders: Natural Earth via `world-atlas`. Country metadata:
`world-countries`. Restaurant data: © OpenStreetMap contributors (ODbL) when using
the `osm` provider.
