# Deploy

## Single binary

`npm run build` in `web/`, then `cargo run --release` in `backend/` serves the app
and the API together on port 8080 (the backend serves `STATIC_DIR`, default
`../web/build`).

## Fly

```sh
fly deploy --build-arg GIT_COMMIT=$(git describe --always --dirty)
```

The `Dockerfile` builds the web app and the backend into one image. `.git` isn't
copied in, hence `GIT_COMMIT`: the commit shows at the bottom of the web and mobile
apps, next to the server's, to tell old builds apart.
