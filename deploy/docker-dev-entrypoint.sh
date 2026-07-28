#!/usr/bin/env bash
set -euo pipefail

WORKSPACE_ROOT="/workspace"
FRONTEND_ROOT="${WORKSPACE_ROOT}/apps/desktop"
FRONTEND_DIST_INDEX="${FRONTEND_ROOT}/dist/web/index.html"
FRONTEND_READY_MARKER="${FRONTEND_ROOT}/dist/.gateway-web-ready"
NODE_MODULES_STAMP="${FRONTEND_ROOT}/node_modules/.gateway-package-lock.sha256"
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0

log() {
  printf '[gateway-dev] %s\n' "$*"
}

ensure_runtime_layout() {
  mkdir -p /data/state
  if [[ ! -f /data/routes.yaml ]]; then
    if [[ -f "${WORKSPACE_ROOT}/routes.yaml" ]]; then
      cp "${WORKSPACE_ROOT}/routes.yaml" /data/routes.yaml
      log "seeded /data/routes.yaml from repository routes.yaml"
    else
      cp "${WORKSPACE_ROOT}/routes.example.yaml" /data/routes.yaml
      log "seeded /data/routes.yaml from repository routes.example.yaml"
    fi
  fi
}

ensure_frontend_dependencies() {
  local lock_hash=""
  lock_hash="$(sha256sum "${FRONTEND_ROOT}/package-lock.json" | awk '{print $1}')"
  if [[ ! -d "${FRONTEND_ROOT}/node_modules" ]] || [[ ! -f "${NODE_MODULES_STAMP}" ]] || [[ "$(cat "${NODE_MODULES_STAMP}")" != "${lock_hash}" ]]; then
    log "installing desktop UI dependencies"
    npm ci --prefix "${FRONTEND_ROOT}" --no-audit --no-fund
    printf '%s' "${lock_hash}" > "${NODE_MODULES_STAMP}"
  else
    log "desktop UI dependencies already match package-lock.json"
  fi
}

wait_for_frontend_dist() {
  local attempts=0
  until [[ -f "${FRONTEND_DIST_INDEX}" && -f "${FRONTEND_READY_MARKER}" ]]; do
    attempts=$((attempts + 1))
    if [[ ${attempts} -gt 180 ]]; then
      log "timed out waiting for a complete frontend publish"
      return 1
    fi
    sleep 1
  done
  log "frontend dist is ready"
}

cleanup_frontend_staging() {
  local dist_root="${FRONTEND_ROOT}/dist"
  if [[ ! -d "${dist_root}" ]]; then
    return 0
  fi

  if ! find "${dist_root}" -mindepth 1 -maxdepth 1 -type d \
    \( -name "web-staging" -o -name "web-staging-*" \) \
    -exec rm -rf -- {} +; then
    log "warning: failed to remove one or more stale frontend staging directories"
  fi
}

cleanup() {
  local exit_code=$?
  trap - EXIT INT TERM
  for pid_var in gateway_watch_pid frontend_watch_pid; do
    local pid="${!pid_var:-}"
    if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
      kill "${pid}" 2>/dev/null || true
      wait "${pid}" 2>/dev/null || true
    fi
  done
  cleanup_frontend_staging
  exit "${exit_code}"
}

main() {
  trap cleanup EXIT INT TERM

  cd "${WORKSPACE_ROOT}"
  log "using CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS} CARGO_INCREMENTAL=${CARGO_INCREMENTAL}"
  ensure_runtime_layout
  ensure_frontend_dependencies
  cleanup_frontend_staging

  rm -f "${FRONTEND_READY_MARKER}"
  export GATEWAY_PREBUILT_WEB_UI=1
  export WATCHPACK_POLLING=true
  export CHOKIDAR_USEPOLLING=1

  log "starting frontend build watcher"
  npm run build:web --prefix apps/desktop -- --watch &
  frontend_watch_pid=$!

  wait_for_frontend_dist

  log "starting gateway cargo watcher"
  cargo watch --poll \
    --watch build.rs \
    --watch Cargo.toml \
    --watch Cargo.lock \
    --watch src \
    --watch apps/desktop/dist/.gateway-web-ready \
    --ignore target \
    -x 'run --locked --bin gateway' &
  gateway_watch_pid=$!

  wait -n "${frontend_watch_pid}" "${gateway_watch_pid}"
}

main "$@"
