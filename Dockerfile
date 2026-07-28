FROM node:22-bookworm-slim AS ui-builder

ARG GATEWAY_AUDIT_NONCE=""

RUN node -e 'const minimum = [22, 22, 0]; const current = process.versions.node.split(".").map(Number); const difference = current.findIndex((value, index) => value !== minimum[index]); if (difference >= 0 && current[difference] < minimum[difference]) { throw new Error("Node.js >=22.22.0 is required; found " + process.versions.node); }'

WORKDIR /app

COPY apps/desktop/package.json apps/desktop/package-lock.json ./apps/desktop/
RUN --mount=type=cache,target=/root/.npm \
  npm ci --prefix apps/desktop --no-audit --no-fund
RUN printf '%s\n' "${GATEWAY_AUDIT_NONCE}" >/dev/null \
  && npm run audit:prod --prefix apps/desktop
COPY apps/desktop ./apps/desktop
RUN GATEWAY_WEB_PRUNE_LIVE=1 npm run build:web --prefix apps/desktop

FROM rust:1.91.1-bookworm AS builder

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends cmake pkg-config clang \
  && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock build.rs ./
COPY .cargo/config.toml ./.cargo/config.toml
COPY apps/desktop ./apps/desktop
COPY --from=ui-builder /app/apps/desktop/dist/web ./apps/desktop/dist/web
COPY --from=ui-builder /app/apps/desktop/dist/.gateway-web-ready ./apps/desktop/dist/.gateway-web-ready
COPY src ./src
COPY examples ./examples
COPY tests ./tests
COPY manifests ./manifests
COPY routes.yaml routes.example.yaml ./

ENV GATEWAY_PREBUILT_WEB_UI=1 \
  CARGO_BUILD_JOBS=1 \
  CARGO_INCREMENTAL=0

RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/usr/local/cargo/git \
  cargo build --locked --release --bin gateway

FROM node:22-bookworm-slim

ARG GATEWAY_AUDIT_NONCE=""

RUN node -e 'const minimum = [22, 22, 0]; const current = process.versions.node.split(".").map(Number); const difference = current.findIndex((value, index) => value !== minimum[index]); if (difference >= 0 && current[difference] < minimum[difference]) { throw new Error("Node.js >=22.22.0 is required; found " + process.versions.node); }'

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    chromium \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/gateway /usr/local/bin/gateway
COPY deploy/docker-entrypoint.sh /usr/local/bin/gateway-entrypoint
COPY routes.yaml routes.example.yaml ./
COPY manifests ./manifests
COPY scripts ./scripts

RUN cd /app/scripts \
  && npm ci --omit=dev --no-audit --no-fund

RUN printf '%s\n' "${GATEWAY_AUDIT_NONCE}" >/dev/null \
  && npm run audit:prod --prefix /app/scripts \
  && npm cache clean --force \
  && chmod +x /usr/local/bin/gateway-entrypoint \
  && mkdir -p /data/state

ENV GATEWAY_RUNTIME_ROLE=standalone

ENV PORT=4200 \
  GATEWAY_ROUTES_FILE=/data/routes.yaml \
  GATEWAY_STATE_DIR=/data/state

ENTRYPOINT ["gateway-entrypoint"]
CMD ["gateway"]
