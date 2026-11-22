# API

Every route, parameter and response is described in the OpenAPI 3.1 document,
served at `GET /api/openapi.json` and checked in as
[`contract/openapi.json`](../contract/openapi.json). It is generated from the handlers
with [utoipa](https://github.com/juhaku/utoipa), and an API change shows up in that
file's diff. The web app's types (`web/src/lib/api-types.ts`) are generated from it:
run `npm run api-types` in `web/` after changing the API.

## Wire conventions

JSON field names are snake_case on the wire (the TypeScript types spell them
raw, Kotlin maps them with `@SerialName`, Swift with `convertFromSnakeCase`).
Optional fields are always sent, as `null` when empty. Timestamps are Unix seconds,
dates are `YYYY-MM-DD`.

## Errors

Every error, including unknown `/api` paths, is
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

## Sessions and accounts

Accounts are anonymous: each device asks `POST /api/session` for a token once and
sends it as `Authorization: Bearer <token>`. Each device has its own session, listed
(and signed out) under `GET /api/account`.

To keep progress after clearing the browser or switching phones, an account can make a
**recovery code** (20 characters, only its hash is stored, a new one replaces the old)
or a **link code** (8 characters, one use, 10 minutes) for another device.
`POST /api/session/restore` takes either; sent with the device's current token,
whatever that account holds is merged into the restored one.

`DELETE /api/account` deletes the account and everything in it.
