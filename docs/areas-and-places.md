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

Finding the city and postal code around a GPS position is answered from the cached
outlines when both are among them, and otherwise by Nominatim, whose answer is kept
in memory for a day per cell of about 100 m. So people opening the app in a place
someone has played before don't queue behind Nominatim's one request per second.
A submitted search (`GET /api/areas`) is kept in memory the same way, by what was typed
in lower case with single spaces: a day when it found something, ten minutes when it
found nothing. Suggestions while typing come from Photon and never wait on Nominatim.

What is left does go to Nominatim, and is counted per client address (the one Fly's
proxy reports in `Fly-Client-IP`; an IPv6 address counts as its /64), so a client that
loops uses up its own lookups rather than everyone's turns. Answers from memory or the
cache are not counted:

| Lookup | Per minute | Per hour |
|---|---|---|
| A new search | 10 | 60 |
| A position in a new cell, outside the cached outlines | 5 | 30 |
| The outline of an area nobody opened before | 20 | 100 |

Past that the answer is a 429 `too_many_requests`. So is the answer to a lookup that
waited 10 seconds for its turn at Nominatim without getting it: about ten are waiting
then, and the client can try again in a moment. A request Nominatim doesn't answer
within 15 seconds fails with `upstream`, and the next one gets its turn. The counts and
the memory live in the one server process and start empty after a restart.

## From a place a player adds to a place on OpenStreetMap

Four tables hold restaurants:

| Table | Holds |
| --- | --- |
| `places` | What players see and check in at: places found on OpenStreetMap, and places players added that OpenStreetMap doesn't have yet (pending: id `user-…`, `added_by` set). |
| `osm_places` | The local copy of OpenStreetMap. Emptied and refilled by the daily import; also updated when a player refreshes a place or looks again nearby. |
| `unknown_places` | Named restaurants in the import box with no `cuisine` tag. Replaced at each import. Nobody can play them. |
| `additions` | One row per place a player added, with its reward. It outlives the pending `places` row. |

A player adds a place with `POST /api/places`, at most 5 a day:

1. A pending row goes into `places` and into the cached areas whose outline holds it.
   Only the player who added it sees it.
2. An `additions` row records the bonus it claims, judged at that moment against the
   places everyone sees in its city (never a postal code): `country` when none serves
   that country, `continent` when none serves that continent. With no cached city
   around it there is no bonus.
3. Unless the request says `note: false` (the player maps it themselves), an
   OpenStreetMap note at that spot asks mappers to add it, with the tags to copy. The
   note's id is kept in `places.osm_note_id`.
4. Nothing is paid yet: `/api/additions` lists it as `pending`, with the XP it would earn.

At each daily import, and when a player looks again nearby
(`POST /api/places/nearby/refresh`), every pending place, anyone's, is compared with
`osm_places`. Its twin is the nearest place within 50 m that serves the same country
and has a similar name: all the words of one name appear in the other, ignoring case
and punctuation. When there is one:

1. The addition is confirmed and points at the twin. The first addition confirmed for
   a twin is rewarded; a later one for the same twin is a `duplicate` and earns nothing.
2. The bonuses of its city are handed out again in the order the places were added: an
   addition keeps the bonus it claimed unless an earlier one, pending or rewarded,
   holds it. A bonus already granted is never lowered.
3. The twin gets a `places` row, the pending place's visits, closed reports and areas
   move to it, and the pending row is deleted. Now everyone sees the place.

XP counts only rewarded additions, so it is paid at confirmation: 30 XP, plus 100 for a
new country, plus another 250 for a new continent. `/api/additions` shows the
confirmation as unseen until `PUT /api/additions/seen` says the player was told.

### Restaurants with unknown food

`GET /api/unknown-places` lists the `unknown_places` in an area. A player can answer
for one in two ways:

- `POST /api/unknown-places/{place_id}/cuisine` says which country's food it serves. This
  adds a pending place as above, with the unknown place's name and position and with
  `suggested_for` set to its id; the note asks mappers to tag the cuisine. It is
  confirmed like any other pending place. If OpenStreetMap gives that place a cuisine
  and no twin matches (another country, say), OpenStreetMap wins: the pending place is
  swapped for the OpenStreetMap one and the addition is deleted, so it earns nothing.
- `PUT /api/unknown-places/{place_id}/no-cuisine` says it serves no particular country's
  food.

The list leaves out what the player already answered for, and what two players said
serves nothing specific. For everyone else the place becomes playable only once
OpenStreetMap has a national cuisine for it: the import keeps those places in
`osm_places`, and only places with no `cuisine` tag in `unknown_places`.
