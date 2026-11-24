# Deploy

## Single binary

`npm run build` in `web/`, then `cargo run --release` in `backend/` serves the app
and the API together on port 8080 (the backend serves `STATIC_DIR`, default
`../web/build`).

## Fly

The app runs on Fly.io as `mangiamundo`, in Frankfurt (`fly.toml`). The
`Dockerfile` builds the web app and the backend into one image.

Every push to `main` is deployed by Fly's GitHub app. It is set up in the Fly
dashboard, so there is no deploy workflow in `.github/workflows`. Each deploy shows up
as a `Fly.io` check on the commit and under the repository's deployments.

The deploy starts at the push, at the same time as the tests in
`.github/workflows/test.yml`. It does not wait for them, and `main` has no required
checks, so a commit that fails its tests is deployed all the same.

A deploy by hand still works:

```sh
fly deploy --build-arg GIT_COMMIT=$(git describe --always --dirty)
```

### The commit label

The commit shows at the bottom of the web and mobile apps, next to the server's, to
tell old builds apart. `.git` isn't copied into the image, so the build takes it from
the `GIT_COMMIT` build argument.

Nothing in the repository passes `GIT_COMMIT` to the deploys from GitHub: `fly.toml`
can only hold fixed build arguments, not the commit being built. Without it
`/api/health` answers `"commit": null` and the web app shows its build time instead
of a commit. A deploy by hand, as above, sets it.

## Upgrading

Migrations run when the server starts. Migration 0017 keeps session tokens as hashes:
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
