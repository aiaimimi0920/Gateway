#!/usr/bin/env bash
set -euo pipefail

WORKSPACE_ROOT="/workspace"
FRONTEND_ROOT="${WORKSPACE_ROOT}/apps/desktop"
SCRIPTS_ROOT="${WORKSPACE_ROOT}/scripts"
FRONTEND_DIST_INDEX="${FRONTEND_ROOT}/dist/web/index.html"
FRONTEND_READY_MARKER="${FRONTEND_ROOT}/dist/.gateway-web-ready"
FRONTEND_NODE_MODULES_STAMP="${FRONTEND_ROOT}/node_modules/.gateway-package-lock.sha256"
SCRIPTS_NODE_MODULES_STAMP="${SCRIPTS_ROOT}/node_modules/.gateway-package-lock.sha256"
: "${GATEWAY_DEV_CARGO_BUILD_JOBS:=4}"
: "${GATEWAY_DEV_CARGO_INCREMENTAL:=1}"
: "${GATEWAY_DEV_RUN_AUDIT:=0}"
: "${GATEWAY_DEV_WATCH:=0}"
: "${GATEWAY_DEV_MODE:=all}"
export CARGO_BUILD_JOBS="${GATEWAY_DEV_CARGO_BUILD_JOBS}"
export CARGO_INCREMENTAL="${GATEWAY_DEV_CARGO_INCREMENTAL}"

log() {
  printf '[gateway-dev] %s\n' "$*"
}

assert_supported_node_version() {
  node -e 'const minimum = [22, 22, 0]; const current = process.versions.node.split(".").map(Number); const difference = current.findIndex((value, index) => value !== minimum[index]); if (difference >= 0 && current[difference] < minimum[difference]) { throw new Error("Node.js >=22.22.0 is required; found " + process.versions.node); }'
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

ensure_node_dependencies() {
  local package_root="$1"
  local stamp_path="$2"
  local package_label="$3"
  local lock_hash=""
  lock_hash="$(sha256sum "${package_root}/package-lock.json" | awk '{print $1}')"
  if [[ ! -d "${package_root}/node_modules" ]] || [[ ! -f "${stamp_path}" ]] || [[ "$(cat "${stamp_path}")" != "${lock_hash}" ]]; then
    log "installing ${package_label} dependencies"
    npm ci --prefix "${package_root}" --no-audit --no-fund
    printf '%s' "${lock_hash}" > "${stamp_path}"
  else
    log "${package_label} dependencies already match package-lock.json"
  fi
}

audit_production_dependencies() {
  local package_root="$1"
  local package_label="$2"
  if [[ "${GATEWAY_DEV_RUN_AUDIT}" != "1" ]]; then
    log "skipping production dependency audit in dev entrypoint for ${package_label}"
    return 0
  fi
  log "auditing ${package_label} production dependencies"
  npm run audit:prod --prefix "${package_root}"
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

run_watch_mode() {
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

prepare_build_environment() {
  assert_supported_node_version
  ensure_node_dependencies "${SCRIPTS_ROOT}" "${SCRIPTS_NODE_MODULES_STAMP}" "browser worker"
  ensure_node_dependencies "${FRONTEND_ROOT}" "${FRONTEND_NODE_MODULES_STAMP}" "desktop UI"
  audit_production_dependencies "${SCRIPTS_ROOT}" "browser worker"
  audit_production_dependencies "${FRONTEND_ROOT}" "desktop UI"
  cleanup_frontend_staging
  rm -f "${FRONTEND_READY_MARKER}"
  export GATEWAY_PREBUILT_WEB_UI=1
}

build_once() {
  log "building frontend once for stable runtime"
  npm run build:web --prefix apps/desktop
  wait_for_frontend_dist

  log "building gateway once for stable runtime"
  cargo build --locked --bin gateway
  cleanup_frontend_staging
}

start_stable_runtime() {
  if [[ ! -x "${WORKSPACE_ROOT}/target/debug/gateway" ]]; then
    log "stable Gateway binary is missing; run the gateway-build service first"
    return 1
  fi
  wait_for_frontend_dist
  log "starting stable gateway without polling watchers"
  trap - EXIT INT TERM
  exec "${WORKSPACE_ROOT}/target/debug/gateway"
}

main() {
  trap cleanup EXIT INT TERM

  cd "${WORKSPACE_ROOT}"
  log "mode=${GATEWAY_DEV_MODE} watch=${GATEWAY_DEV_WATCH} CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS} CARGO_INCREMENTAL=${CARGO_INCREMENTAL}"
  ensure_runtime_layout

  if [[ "${GATEWAY_DEV_MODE}" == "build" ]]; then
    prepare_build_environment
    build_once
    log "build-only mode completed"
    return 0
  fi

  if [[ "${GATEWAY_DEV_MODE}" == "run" ]]; then
    if [[ "${GATEWAY_DEV_WATCH}" == "1" ]]; then
      prepare_build_environment
      run_watch_mode
    else
      start_stable_runtime
    fi
    return 0
  fi

  if [[ "${GATEWAY_DEV_MODE}" != "all" ]]; then
    log "unsupported GATEWAY_DEV_MODE=${GATEWAY_DEV_MODE}; expected build, run, or all"
    return 1
  fi

  prepare_build_environment
  if [[ "${GATEWAY_DEV_WATCH}" == "1" ]]; then
    run_watch_mode
  else
    build_once
    start_stable_runtime
  fi
}

main "$@"
