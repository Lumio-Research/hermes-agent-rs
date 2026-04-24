#!/usr/bin/env bash
set -euo pipefail

retry() {
  local attempts="$1"
  shift
  local n=1
  until "$@"; do
    if [[ "$n" -ge "$attempts" ]]; then
      echo "Command failed after $attempts attempts: $*" >&2
      return 1
    fi
    local sleep_s=$(( n * 2 ))
    echo "Retry $n/$attempts failed, retrying in ${sleep_s}s: $*" >&2
    sleep "$sleep_s"
    n=$(( n + 1 ))
  done
}

validate_secret_line() {
  local line="$1"
  [[ "$line" =~ ^[A-Z][A-Z0-9_]*=.+$ ]]
}

usage() {
  cat <<'EOF'
Usage:
  provision-tenant.sh \
    --template charts/fly/fly.toml.tmpl \
    --app-name hermes-tenant-acme \
    --region hkg \
    --image ghcr.io/your-org/hermes-runtime:v0.1.0 \
    --tenant-id acme \
    --tenant-name "ACME Support" \
    --environment prod \
    --log-level info \
    --hermes-home /data/hermes \
    --public-base-url https://support-acme.example.com \
    --volume-name hermes-kb-acme \
    --volume-size-gb 10 \
    --internal-port 8080 \
    --min-machines 1 \
    --vm-size shared-cpu-1x \
    --secrets-file charts/fly/secrets.example.env \
    [--domain support-acme.example.com] \
    [--output-dir .fly-generated/acme]

secrets-file format:
  KEY=VALUE
  KEY2=VALUE2
EOF
}

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RENDER_SCRIPT="$SCRIPT_DIR/render-fly-toml.sh"

TEMPLATE=""
APP_NAME=""
REGION=""
IMAGE=""
TENANT_ID=""
TENANT_NAME=""
ENVIRONMENT=""
LOG_LEVEL="info"
HERMES_HOME="/data/hermes"
PUBLIC_BASE_URL=""
VOLUME_NAME=""
VOLUME_SIZE_GB="10"
INTERNAL_PORT="8080"
MIN_MACHINES="1"
VM_SIZE="shared-cpu-1x"
SECRETS_FILE=""
DOMAIN=""
OUTPUT_DIR=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --template) TEMPLATE="$2"; shift 2 ;;
    --app-name) APP_NAME="$2"; shift 2 ;;
    --region) REGION="$2"; shift 2 ;;
    --image) IMAGE="$2"; shift 2 ;;
    --tenant-id) TENANT_ID="$2"; shift 2 ;;
    --tenant-name) TENANT_NAME="$2"; shift 2 ;;
    --environment) ENVIRONMENT="$2"; shift 2 ;;
    --log-level) LOG_LEVEL="$2"; shift 2 ;;
    --hermes-home) HERMES_HOME="$2"; shift 2 ;;
    --public-base-url) PUBLIC_BASE_URL="$2"; shift 2 ;;
    --volume-name) VOLUME_NAME="$2"; shift 2 ;;
    --volume-size-gb) VOLUME_SIZE_GB="$2"; shift 2 ;;
    --internal-port) INTERNAL_PORT="$2"; shift 2 ;;
    --min-machines) MIN_MACHINES="$2"; shift 2 ;;
    --vm-size) VM_SIZE="$2"; shift 2 ;;
    --secrets-file) SECRETS_FILE="$2"; shift 2 ;;
    --domain) DOMAIN="$2"; shift 2 ;;
    --output-dir) OUTPUT_DIR="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$TEMPLATE" || -z "$APP_NAME" || -z "$REGION" || -z "$IMAGE" || -z "$TENANT_ID" || -z "$TENANT_NAME" || -z "$ENVIRONMENT" || -z "$PUBLIC_BASE_URL" || -z "$VOLUME_NAME" || -z "$SECRETS_FILE" ]]; then
  echo "Missing required arguments." >&2
  usage
  exit 1
fi

if [[ -z "$OUTPUT_DIR" ]]; then
  OUTPUT_DIR=".fly-generated/$TENANT_ID"
fi

if ! command -v fly >/dev/null 2>&1; then
  echo "fly CLI is required but not found in PATH." >&2
  exit 1
fi

if ! fly auth whoami >/dev/null 2>&1; then
  echo "Fly auth is missing. Please run: fly auth login" >&2
  exit 1
fi

if ! command -v rg >/dev/null 2>&1; then
  echo "rg is required but not found in PATH." >&2
  exit 1
fi

if [[ ! -f "$SECRETS_FILE" ]]; then
  echo "Secrets file not found: $SECRETS_FILE" >&2
  exit 1
fi

mkdir -p "$OUTPUT_DIR"
RENDERED_TOML="$OUTPUT_DIR/fly.toml"

echo "[1/7] Rendering fly.toml"
"$RENDER_SCRIPT" \
  --template "$TEMPLATE" \
  --output "$RENDERED_TOML" \
  --app-name "$APP_NAME" \
  --region "$REGION" \
  --image "$IMAGE" \
  --tenant-id "$TENANT_ID" \
  --tenant-name "$TENANT_NAME" \
  --environment "$ENVIRONMENT" \
  --log-level "$LOG_LEVEL" \
  --hermes-home "$HERMES_HOME" \
  --public-base-url "$PUBLIC_BASE_URL" \
  --volume-name "$VOLUME_NAME" \
  --internal-port "$INTERNAL_PORT" \
  --min-machines "$MIN_MACHINES" \
  --vm-size "$VM_SIZE"

echo "[2/7] Ensuring Fly app exists"
if fly apps show "$APP_NAME" >/dev/null 2>&1; then
  echo "App exists: $APP_NAME"
else
  fly apps create "$APP_NAME"
fi

echo "[3/7] Ensuring Fly volume exists"
if fly volumes list -a "$APP_NAME" | rg "^${VOLUME_NAME}[[:space:]]" >/dev/null 2>&1; then
  echo "Volume exists: $VOLUME_NAME"
else
  fly volumes create "$VOLUME_NAME" --region "$REGION" --size "$VOLUME_SIZE_GB" -a "$APP_NAME"
fi

echo "[4/7] Setting Fly secrets"
declare -a secret_pairs=()
while IFS= read -r line; do
  line="${line%%$'\r'}"
  [[ -z "${line// }" ]] && continue
  [[ "$line" =~ ^[[:space:]]*# ]] && continue
  if ! validate_secret_line "$line"; then
    echo "Invalid secret line format: $line" >&2
    exit 1
  fi
  if [[ "$line" =~ =replace_me$ ]]; then
    echo "Placeholder secret detected (replace_me): $line" >&2
    exit 1
  fi
  secret_pairs+=("$line")
done < "$SECRETS_FILE"

if [[ ${#secret_pairs[@]} -eq 0 ]]; then
  echo "No secrets found in: $SECRETS_FILE" >&2
  exit 1
fi

fly secrets set -a "$APP_NAME" "${secret_pairs[@]}"

echo "[5/7] Deploying tenant runtime"
retry 3 fly deploy -a "$APP_NAME" --config "$RENDERED_TOML" --image "$IMAGE"

echo "[6/7] Binding custom domain (optional)"
if [[ -n "$DOMAIN" ]]; then
  if fly certs show "$DOMAIN" -a "$APP_NAME" >/dev/null 2>&1; then
    echo "Domain cert exists: $DOMAIN"
  else
    fly certs add "$DOMAIN" -a "$APP_NAME"
  fi
else
  echo "No custom domain provided, skipping."
fi

echo "[7/7] Checking status"
fly status -a "$APP_NAME"

echo "Provision complete for tenant: $TENANT_ID"
echo "Rendered config: $RENDERED_TOML"
