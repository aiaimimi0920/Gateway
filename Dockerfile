FROM rust:1.91.1-bookworm AS builder

WORKDIR /app

RUN apt-get update \
  && apt-get install -y --no-install-recommends cmake pkg-config clang \
  && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY examples ./examples
COPY tests ./tests
COPY manifests ./manifests
COPY routes.yaml routes.example.yaml ./

RUN --mount=type=cache,target=/usr/local/cargo/registry \
  --mount=type=cache,target=/usr/local/cargo/git \
  cargo build --locked --release --bin neuro-gateway

FROM node:20-bookworm-slim

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
    ca-certificates \
    chromium \
  && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/neuro-gateway /usr/local/bin/neuro-gateway
COPY routes.yaml routes.example.yaml ./
COPY manifests ./manifests
COPY scripts ./scripts

RUN cd /app/scripts \
  && npm ci --omit=dev \
  && npm cache clean --force

ENV PORT=4200

CMD ["neuro-gateway"]
