# Deploy

## Single binary

`npm run build` in `web/`, then `cargo run --release` in `backend/` serves the app
and the API together on port 8080 (the backend serves `STATIC_DIR`, default
`../web/build`).

## Fly

The app runs on Fly.io as `mangiamundo`, in Frankfurt (`fly.toml`). The
`Dockerfile` builds the web app and the backend into one image.

The `deploy` job in `.github/workflows/test.yml` deploys every push to `main`, after
the `backend`, `web` and `android` jobs of that push have passed. A commit that fails
them is not deployed. The iOS tests (`ios.yml`) are not waited for: they run on slower
macOS runners, only when the iOS app changes, and Fly does not deploy that app.

Deploys run one at a time. A run that finds `main` has moved on while it waited
deploys nothing, and leaves it to the run of the newer commit.

The job needs a `FLY_API_TOKEN` repository secret. Without it the job passes and
deploys nothing.

### Setting it up, once

Until this is done, Fly's GitHub app deploys every push to `main` as it did before:
at the push, without waiting for the tests, and without the commit label.

1. Make a deploy token for the app: `fly tokens create deploy -a mangiamundo`.
2. Add it on GitHub as a repository secret named `FLY_API_TOKEN` (Settings, Secrets
   and variables, Actions), with the whole output, `FlyV1 ` included.
3. Disconnect the repository from the app in the Fly dashboard, so that Fly stops
   deploying by itself. With both in place every push is deployed twice, one of
   them without waiting for the tests.
4. Optional: in a branch protection rule or ruleset for `main`, require the
   `backend`, `web` and `android` checks, so a pull request that fails them cannot be
   merged.

A deploy by hand still works:

```sh
fly deploy --build-arg GIT_COMMIT=$(git describe --always --dirty)
```

### The commit label

The commit shows at the bottom of the web and mobile apps, next to the server's, to
tell old builds apart. `.git` isn't copied into the image, so the build takes it from
the `GIT_COMMIT` build argument: the web build reads it in `vite.config.ts`, the
backend at compile time for `/api/health`.

The `deploy` job passes the short commit hash, and so does a deploy by hand, as
above. `fly.toml` can only hold fixed build arguments, not the commit being built, so
a deploy by Fly's GitHub app has none: `/api/health` answers `"commit": null` and the
web app shows its build time instead of a commit.

## Upgrading

Migrations run when the server starts. Migration 0018 keeps session tokens as hashes:
the server hashes the ones already there at its first start after it (devices keep their
tokens), and a server from before it can no longer read the `sessions` table. To go back
to an older build, restore a snapshot from before the upgrade.

New accounts are limited per address, which the server reads from the `Fly-Client-IP`
header that Fly's proxy sets. Behind another proxy that header is absent (only the limit
for everyone together applies) unless the proxy sets it; it must then also overwrite one a
client sends.

## Backups

All data is one SQLite file, `/data/mangiamundo.db`, on one Fly volume named `data`.
Nothing in the repository makes or tests a backup.

- Fly takes a snapshot of every volume once a day and keeps it for 5 days unless the
  volume is set otherwise. That is the only copy there is, and it lives with Fly.
  `fly volumes list` and `fly volumes snapshots list <volume id>` show them;
  `fly volumes create data --snapshot-id <id>` restores one into a new volume. That
  volume is not attached to anything: the running Machine keeps the old one until a
  Machine is started on the new volume and the old one is stopped, so that only the
  restored copy takes writes. Fly's volume documentation has the steps.
  Snapshots can be kept for up to 60 days. `snapshot_retention` under `[[mounts]]` in
  `fly.toml` only counts for a volume a deploy creates; for the `data` volume that
  exists, the retention is changed on the volume itself.
- For a copy outside Fly: `sqlite3 /data/mangiamundo.db ".backup /data/backup.db"`
  makes a consistent copy while the server runs, to fetch with `fly ssh sftp get`.
  The image has no `sqlite3`, so it has to be installed first.
- [Litestream](https://litestream.io) copies every change to object storage as it
  happens. It is not set up.

A snapshot can be up to a day old, so what was written since is lost on a restore.
