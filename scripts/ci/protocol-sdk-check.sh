#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SDK_DIR="$ROOT_DIR/sdk/typescript"

if [[ ! -f "$SDK_DIR/package.json" ]]; then
  echo "protocol SDK package not found at $SDK_DIR, skipping"
  exit 0
fi

echo "[protocol-sdk] install deps"
cd "$SDK_DIR"
pnpm install --frozen-lockfile || pnpm install

echo "[protocol-sdk] build"
pnpm build

echo "[protocol-sdk] contract tests"
pnpm test

if [[ -n "${HERMES_E2E_BASE:-}" ]]; then
  echo "[protocol-sdk] e2e smoke enabled against $HERMES_E2E_BASE"
  pnpm test:e2e
else
  echo "[protocol-sdk] e2e smoke skipped (set HERMES_E2E_BASE to enable)"
fi
