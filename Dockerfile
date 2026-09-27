FROM node:26-bookworm-slim AS web

WORKDIR /source

COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
RUN npm ci

COPY apps/web apps/web
RUN npm run build --workspace @opsscope/web

FROM rust:1.98.1-bookworm AS server

WORKDIR /source

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps/desktop apps/desktop
COPY apps/server apps/server
COPY crates/opsscope-core crates/opsscope-core
RUN cargo build --locked --release -p opsscope-server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system opsscope \
    && useradd --system --gid opsscope --home-dir /nonexistent --shell /usr/sbin/nologin opsscope \
    && mkdir --parents /data /opt/opsscope/web \
    && chown opsscope:opsscope /data

COPY --from=server /source/target/release/opsscope-server /usr/local/bin/opsscope-server
COPY --from=web /source/apps/web/dist /opt/opsscope/web

USER opsscope

ENV OPSSCOPE_BIND_ADDRESS=0.0.0.0:4317 \
    OPSSCOPE_DATA_DIR=/data \
    OPSSCOPE_WEB_DIR=/opt/opsscope/web

VOLUME ["/data"]
EXPOSE 4317

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "--fail", "--silent", "http://127.0.0.1:4317/api/health"]

ENTRYPOINT ["opsscope-server"]
