FROM node:26-bookworm-slim AS web

WORKDIR /source

COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
RUN npm ci

COPY apps/web apps/web
RUN npm run build --workspace @opscope/web

FROM rust:1.98.1-bookworm AS server

WORKDIR /source

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY apps/desktop apps/desktop
COPY apps/server apps/server
COPY crates/opscope-core crates/opscope-core
RUN cargo build --locked --release -p opscope-server

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system opscope \
    && useradd --system --gid opscope --home-dir /nonexistent --shell /usr/sbin/nologin opscope \
    && mkdir --parents /data /opt/opscope/web \
    && chown opscope:opscope /data

COPY --from=server /source/target/release/opscope-server /usr/local/bin/opscope-server
COPY --from=web /source/apps/web/dist /opt/opscope/web

USER opscope

ENV OPSCOPE_BIND_ADDRESS=0.0.0.0:4317 \
    OPSCOPE_DATA_DIR=/data \
    OPSCOPE_WEB_DIR=/opt/opscope/web

VOLUME ["/data"]
EXPOSE 4317

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["curl", "--fail", "--silent", "http://127.0.0.1:4317/api/health"]

ENTRYPOINT ["opscope-server"]
