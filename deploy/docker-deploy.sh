#!/usr/bin/env bash
# =============================================================================
# Gateway Docker Deployment Preparation Script
# =============================================================================
# Inspired by the recommended Sub2API server-side deployment flow:
#   - prefer docker-compose.local.yml for host-visible persistent data
#   - generate required secrets on first deploy
#   - create the local data directories up front
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.local.yml"
ENV_TEMPLATE="${SCRIPT_DIR}/.env.example"
ENV_FILE="${SCRIPT_DIR}/.env"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_info() {
  echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
  echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
  echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
  echo -e "${RED}[ERROR]${NC} $1"
}

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

generate_secret() {
  openssl rand -hex 32
}

generate_token() {
  printf 'gateway-%s' "$(openssl rand -hex 16)"
}

read_env_value() {
  local key="$1"
  local line
  line="$(grep -E "^${key}=" "$ENV_FILE" | tail -n 1 || true)"
  if [ -z "$line" ]; then
    return 0
  fi
  printf '%s' "${line#*=}"
}

set_env_value() {
  local key="$1"
  local value="$2"
  if grep -Eq "^${key}=" "$ENV_FILE"; then
    sed -i.bak "s#^${key}=.*#${key}=${value}#" "$ENV_FILE"
  elif grep -Eq "^# *${key}=" "$ENV_FILE"; then
    sed -i.bak "s#^# *${key}=.*#${key}=${value}#" "$ENV_FILE"
  else
    printf '\n%s=%s\n' "$key" "$value" >>"$ENV_FILE"
  fi
}

ensure_env_value() {
  local key="$1"
  local value="$2"
  local current
  current="$(read_env_value "$key")"
  if [ -z "$current" ]; then
    set_env_value "$key" "$value"
    GENERATED_VALUES["$key"]="$value"
  fi
}

cleanup_backup_file() {
  if [ -f "${ENV_FILE}.bak" ]; then
    rm -f "${ENV_FILE}.bak"
  fi
}

main() {
  echo ""
  echo "=========================================="
  echo "  Gateway Docker Deployment Preparation"
  echo "=========================================="
  echo ""

  if ! command_exists docker; then
    print_error "docker is not installed."
    exit 1
  fi
  if ! command_exists openssl; then
    print_error "openssl is not installed."
    exit 1
  fi
  if [ ! -f "$COMPOSE_FILE" ] || [ ! -f "$ENV_TEMPLATE" ]; then
    print_error "deploy directory is incomplete. Expected docker-compose.local.yml and .env.example."
    exit 1
  fi

  if [ ! -f "$ENV_FILE" ]; then
    print_info "Creating .env from .env.example"
    cp "$ENV_TEMPLATE" "$ENV_FILE"
  else
    print_warning ".env already exists; only missing values will be generated."
  fi

  mkdir -p "${SCRIPT_DIR}/gateway_data" "${SCRIPT_DIR}/redis_data"
  print_success "Prepared gateway_data/ and redis_data/"

  declare -gA GENERATED_VALUES=()
  ensure_env_value "GATEWAY_API_KEY" "$(generate_token)"
  ensure_env_value "GATEWAY_API_KEY_SECRET" "$(generate_secret)"
  ensure_env_value "GATEWAY_MANAGEMENT_TOKEN" "$(generate_secret)"
  ensure_env_value "GATEWAY_CONSOLE_REMOTE_ACCESS" "true"

  cleanup_backup_file
  chmod 600 "$ENV_FILE" 2>/dev/null || true

  local bind_host gateway_port
  bind_host="$(read_env_value "GATEWAY_BIND_HOST")"
  gateway_port="$(read_env_value "GATEWAY_PORT")"
  [ -z "$bind_host" ] && bind_host="0.0.0.0"
  [ -z "$gateway_port" ] && gateway_port="4200"

  echo ""
  echo "=========================================="
  echo "  Gateway Docker Deployment Ready"
  echo "=========================================="
  echo ""
  if [ "${#GENERATED_VALUES[@]}" -gt 0 ]; then
    echo "Generated credentials:"
    for key in GATEWAY_API_KEY GATEWAY_API_KEY_SECRET GATEWAY_MANAGEMENT_TOKEN; do
      if [ -n "${GENERATED_VALUES[$key]:-}" ]; then
        printf '  %s=%s\n' "$key" "${GENERATED_VALUES[$key]}"
      fi
    done
    echo ""
  fi

  cat <<EOF
Next steps:
  cd "${SCRIPT_DIR}"
  docker compose -f docker-compose.local.yml up -d
  docker compose -f docker-compose.local.yml logs -f gateway

Gateway API:
  http://${bind_host}:${gateway_port}

Gateway Web Console:
  http://${bind_host}:${gateway_port}/ui/

Local persistent data:
  ${SCRIPT_DIR}/gateway_data
  ${SCRIPT_DIR}/redis_data
EOF
  echo ""
  print_warning "Keep the generated credentials in ${ENV_FILE} secure."
}

main "$@"
