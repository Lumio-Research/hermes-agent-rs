#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage:
  render-fly-toml.sh \
    --template charts/fly/fly.toml.tmpl \
    --output /tmp/acme-fly.toml \
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
    --internal-port 8080 \
    --min-machines 1 \
    --vm-size shared-cpu-1x
EOF
}

TEMPLATE=""
OUTPUT=""
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
INTERNAL_PORT="8080"
MIN_MACHINES="1"
VM_SIZE="shared-cpu-1x"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --template) TEMPLATE="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
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
    --internal-port) INTERNAL_PORT="$2"; shift 2 ;;
    --min-machines) MIN_MACHINES="$2"; shift 2 ;;
    --vm-size) VM_SIZE="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown argument: $1" >&2; usage; exit 1 ;;
  esac
done

if [[ -z "$TEMPLATE" || -z "$OUTPUT" || -z "$APP_NAME" || -z "$REGION" || -z "$IMAGE" || -z "$TENANT_ID" || -z "$TENANT_NAME" || -z "$ENVIRONMENT" || -z "$PUBLIC_BASE_URL" || -z "$VOLUME_NAME" ]]; then
  echo "Missing required arguments." >&2
  usage
  exit 1
fi

IMAGE_REPOSITORY="${IMAGE%:*}"
IMAGE_TAG="${IMAGE##*:}"
if [[ "$IMAGE_REPOSITORY" == "$IMAGE_TAG" ]]; then
  echo "Invalid --image value. Expect repository:tag" >&2
  exit 1
fi

export APP_NAME REGION IMAGE_REPOSITORY IMAGE_TAG LOG_LEVEL HERMES_HOME PUBLIC_BASE_URL TENANT_ID TENANT_NAME ENVIRONMENT VOLUME_NAME INTERNAL_PORT MIN_MACHINES VM_SIZE

python3 - "$TEMPLATE" "$OUTPUT" <<'PY'
import sys
from pathlib import Path
import os

template_path = Path(sys.argv[1])
output_path = Path(sys.argv[2])

content = template_path.read_text(encoding="utf-8")

mapping = {
    "app_name": os.environ["APP_NAME"],
    "region": os.environ["REGION"],
    "image_repository": os.environ["IMAGE_REPOSITORY"],
    "image_tag": os.environ["IMAGE_TAG"],
    "log_level": os.environ["LOG_LEVEL"],
    "hermes_home": os.environ["HERMES_HOME"],
    "public_base_url": os.environ["PUBLIC_BASE_URL"],
    "tenant_id": os.environ["TENANT_ID"],
    "tenant_name": os.environ["TENANT_NAME"],
    "environment": os.environ["ENVIRONMENT"],
    "volume_name": os.environ["VOLUME_NAME"],
    "internal_port": os.environ["INTERNAL_PORT"],
    "min_machines": os.environ["MIN_MACHINES"],
    "vm_size": os.environ["VM_SIZE"],
}

for key, value in mapping.items():
    content = content.replace("{{ " + key + " }}", value)

output_path.parent.mkdir(parents=True, exist_ok=True)
output_path.write_text(content, encoding="utf-8")
PY

echo "Rendered fly.toml -> $OUTPUT"
