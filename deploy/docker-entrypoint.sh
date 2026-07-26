#!/bin/sh
set -eu

ROUTES_FILE="${GATEWAY_ROUTES_FILE:-/data/routes.yaml}"
STATE_DIR="${GATEWAY_STATE_DIR:-/data/state}"
BOOTSTRAP_SOURCE="/app/routes.example.yaml"

mkdir -p "$(dirname "$ROUTES_FILE")"
mkdir -p "$STATE_DIR"

if [ ! -f "$ROUTES_FILE" ]; then
  cp "$BOOTSTRAP_SOURCE" "$ROUTES_FILE"
fi

exec "$@"
