#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MOBILE_DIR="$ROOT_DIR/apps/mobile-app"

echo "[mobile-check] install dependencies"
cd "$MOBILE_DIR"
pnpm install --frozen-lockfile || pnpm install

echo "[mobile-check] TypeScript type check"
pnpm lint

echo "[mobile-check] protocol fixture tests"
pnpm test

echo "[mobile-check] ok"
