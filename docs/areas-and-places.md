# Areas and places

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

## Caching

Results are cached per area (with its outline) and refreshed after `CACHE_TTL_DAYS`.
Belgium and its border towns come from a local copy instead, refreshed from Overpass
once a day with one query for the whole box; each refresh marks the cached areas there
stale, so changes in OpenStreetMap show up within about a day.
