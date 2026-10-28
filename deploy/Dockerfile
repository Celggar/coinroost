# syntax=docker/dockerfile:1
# coinroost-server with its web app, for docker compose (deploy/compose.prod.yml). Build from the repository root:
#   docker build -f deploy/Dockerfile .

ARG NODE_IMAGE=node:24.22.1-alpine
ARG RUST_IMAGE=rust:1.99.1-slim-trixie
ARG RUNTIME_IMAGE=debian:trixie-slim

FROM ${NODE_IMAGE} AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web/ ./
RUN npm run build

FROM ${RUST_IMAGE} AS server
# rust-toolchain.toml also lists the iOS and wasm targets; the image's own toolchain is the same version and
# builds the server alone, so nothing else is downloaded.
ENV RUSTUP_TOOLCHAIN=1.99.1 CARGO_BUILD_JOBS=4
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY spec/ spec/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p coinroost-server \
 && cp target/release/coinroost-server /usr/local/bin/

FROM ${RUNTIME_IMAGE}
RUN useradd --system --uid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin coinroost
COPY --from=server /usr/local/bin/coinroost-server /usr/local/bin/coinroost-server
COPY --from=web /src/web/build /srv/web
USER 10001
ENV COINROOST_BIND=0.0.0.0:8080 COINROOST_WEB_DIR=/srv/web
EXPOSE 8080
ENTRYPOINT ["coinroost-server"]
CMD ["serve"]
