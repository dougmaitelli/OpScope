FROM node:24-bookworm-slim AS web

WORKDIR /source

COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
RUN npm ci

COPY apps/web apps/web
RUN npm run build --workspace @ciwatcher/web

FROM rust:1.98.1-bookworm AS server

WORKDIR /source

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps/desktop apps/desktop
COPY apps/server apps/server
COPY crates/ciwatcher-core crates/ciwatcher-core
RUN cargo build --locked --release -p ciwatcher-server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system ciwatcher \
    && useradd --system --gid ciwatcher --home-dir /nonexistent --shell /usr/sbin/nologin ciwatcher \
    && mkdir --parents /data /opt/ciwatcher/web \
    && chown ciwatcher:ciwatcher /data

COPY --from=server /source/target/release/ciwatcher-server /usr/local/bin/ciwatcher-server
COPY --from=web /source/apps/web/dist /opt/ciwatcher/web

USER ciwatcher

ENV CIWATCHER_BIND_ADDRESS=0.0.0.0:4317 \
    CIWATCHER_DATA_DIR=/data \
    CIWATCHER_WEB_DIR=/opt/ciwatcher/web

VOLUME ["/data"]
EXPOSE 4317

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "--fail", "--silent", "http://127.0.0.1:4317/api/health"]

ENTRYPOINT ["ciwatcher-server"]
