#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

echo "[protocol-matrix] rust fixtures"
cd "$ROOT_DIR"
cargo test -p hermes-transport

echo "[protocol-matrix] typescript fixtures"
cd "$ROOT_DIR/sdk/typescript"
pnpm install --frozen-lockfile || pnpm install
pnpm build
pnpm test

echo "[protocol-matrix] react native fixtures"
cd "$ROOT_DIR/apps/mobile-app"
pnpm install --frozen-lockfile || pnpm install
pnpm test

echo "[protocol-matrix] ok"
