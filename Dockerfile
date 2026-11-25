# The Rust backend serves the built web app, so one image holds both.
# The deploy job in .github/workflows/test.yml builds and deploys this (docs/deploy.md).
# .git is not copied in, so the commit shown in the apps comes from the GIT_COMMIT
# build argument. That job passes it; a deploy by hand can too:
#   fly deploy --build-arg GIT_COMMIT=$(git describe --always --dirty)

FROM node:24-slim AS web
WORKDIR /web
COPY web/package.json web/package-lock.json web/.npmrc ./
RUN npm ci
COPY web/ ./
ARG GIT_COMMIT
RUN npm run build

# The Rust version in rust-toolchain.toml.
FROM rust:1.100-bookworm AS backend
WORKDIR /backend
# The share card draws the apps' country outlines in their font.
# It is included from ../../shared, seen from backend/src.
COPY shared/ /shared/
COPY backend/ ./
ARG GIT_COMMIT
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir /data
COPY --from=backend /backend/target/release/mangiamundo /usr/local/bin/mangiamundo
COPY --from=web /web/build /app/web
ENV BIND=0.0.0.0:8080 \
    DATABASE_URL=sqlite:///data/mangiamundo.db \
    STATIC_DIR=/app/web
EXPOSE 8080
CMD ["mangiamundo"]
