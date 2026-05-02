#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CLIENT_DIR="$ROOT_DIR/apps/web-app"

echo "[electron-check] install merged web+desktop dependencies"
cd "$CLIENT_DIR"
pnpm install --frozen-lockfile || pnpm install
pnpm build:web

echo "[electron-check] build electron shell"
pnpm build:desktop

echo "[electron-check] ok"
