FROM node:22-bookworm-slim AS ui-builder

RUN node -e 'const minimum = [22, 22, 0]; const current = process.versions.node.split(".").map(Number); const difference = current.findIndex((value, index) => value !== minimum[index]); if (difference >= 0 && current[difference] < minimum[difference]) { throw new Error("Node.js >=22.22.0 is required; found " + process.versions.node); }'

WORKDIR /app

COPY apps/desktop/package.json apps/desktop/package-lock.json ./apps/desktop/
RUN --mount=type=cache,target=/root/.npm \
  attempts=0; \
  until [ "$attempts" -ge 3 ]; do \
    npm ci --prefix apps/desktop --no-audit --no-fund --prefer-offline && break; \
    attempts=$((attempts + 1)); \
    if [ "$attempts" -ge 3 ]; then \
      exit 1; \
    fi; \
    sleep "$attempts"; \
  done
ARG GATEWAY_AUDIT_NONCE=""
RUN printf '%s\n' "${GATEWAY_AUDIT_NONCE}" >/dev/null \
  && npm run audit:prod --prefix apps/desktop
COPY apps/desktop ./apps/desktop
RUN GATEWAY_WEB_PRUNE_LIVE=1 npm run build:web --prefix apps/desktop

FROM rust:1.91.1-bookworm AS builder

WORKDIR /app

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
  --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
  apt-get -o Acquire::Retries=3 update \
  && apt-get install -y --no-install-recommends cmake pkg-config clang lld

COPY Cargo.toml Cargo.lock build.rs ./
COPY build_support ./build_support
COPY .cargo/config.toml ./.cargo/config.toml
COPY apps/desktop ./apps/desktop
COPY --from=ui-builder /app/apps/desktop/dist/web ./apps/desktop/dist/web
COPY --from=ui-builder /app/apps/desktop/dist/.gateway-web-ready ./apps/desktop/dist/.gateway-web-ready
COPY src ./src
COPY crates ./crates

ENV GATEWAY_PREBUILT_WEB_UI=1 \
  CARGO_TARGET_DIR=/cargo-target \
  CARGO_BUILD_JOBS=1 \
  CARGO_INCREMENTAL=0 \
  RUSTFLAGS="-C link-arg=-fuse-ld=lld"

RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/usr/local/cargo/git \
  --mount=type=cache,target=/cargo-target \
  cargo build --locked --release --bin gateway \
  && cp /cargo-target/release/gateway /app/gateway-release

FROM node:22-bookworm-slim

RUN node -e 'const minimum = [22, 22, 0]; const current = process.versions.node.split(".").map(Number); const difference = current.findIndex((value, index) => value !== minimum[index]); if (difference >= 0 && current[difference] < minimum[difference]) { throw new Error("Node.js >=22.22.0 is required; found " + process.versions.node); }'

RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
  --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
  apt-get -o Acquire::Retries=3 update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    chromium

WORKDIR /app

COPY deploy/docker-entrypoint.sh /usr/local/bin/gateway-entrypoint
COPY scripts/package.json scripts/package-lock.json ./scripts/

RUN --mount=type=cache,target=/root/.npm \
  cd /app/scripts \
  && attempts=0; \
  until [ "$attempts" -ge 3 ]; do \
    npm ci --omit=dev --no-audit --no-fund --prefer-offline && break; \
    attempts=$((attempts + 1)); \
    if [ "$attempts" -ge 3 ]; then \
      exit 1; \
    fi; \
    sleep "$attempts"; \
  done

ARG GATEWAY_AUDIT_NONCE=""
RUN printf '%s\n' "${GATEWAY_AUDIT_NONCE}" >/dev/null \
  && npm run audit:prod --prefix /app/scripts \
  && npm cache clean --force \
  && chmod +x /usr/local/bin/gateway-entrypoint \
  && mkdir -p /data/state

COPY routes.example.yaml ./routes.yaml
COPY routes.example.yaml ./routes.example.yaml
COPY manifests ./manifests
COPY scripts ./scripts
COPY --from=builder /app/gateway-release /usr/local/bin/gateway

ENV GATEWAY_RUNTIME_ROLE=standalone

ENV PORT=4200 \
  GATEWAY_ROUTES_FILE=/data/routes.yaml \
  GATEWAY_STATE_DIR=/data/state

ENTRYPOINT ["gateway-entrypoint"]
CMD ["gateway"]
