#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SDK_DIR="$ROOT_DIR/sdk/typescript"

if [[ -n "${HERMES_E2E_ADDR:-}" ]]; then
  SERVER_ADDR="$HERMES_E2E_ADDR"
else
  FREE_PORT="$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1]); s.close()')"
  SERVER_ADDR="127.0.0.1:${FREE_PORT}"
fi
SERVER_URL="${HERMES_E2E_BASE:-http://$SERVER_ADDR}"
E2E_TOKEN="${HERMES_E2E_TOKEN:-local-dev-token}"
SERVER_LOG="${ROOT_DIR}/target/protocol-e2e-local.server.log"
SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    echo "[protocol-e2e-local] stopping hermes-server pid=$SERVER_PID"
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "[protocol-e2e-local] building hermes-server"
cd "$ROOT_DIR"
cargo build -p hermes-server >/dev/null

echo "[protocol-e2e-local] starting hermes-server on $SERVER_ADDR"
HERMES_SERVER_ADDR="$SERVER_ADDR" \
HERMES_HTTP_ADDR="$SERVER_ADDR" \
HERMES_HTTP_API_KEY="$E2E_TOKEN" \
target/debug/hermes-server >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

sleep 1
if ! kill -0 "$SERVER_PID" >/dev/null 2>&1; then
  echo "[protocol-e2e-local] hermes-server exited during startup"
  echo "---- server log ----"
  cat "$SERVER_LOG"
  exit 1
fi

for _ in $(seq 1 30); do
  if curl -fsS "$SERVER_URL/health" >/dev/null 2>&1; then
    echo "[protocol-e2e-local] server ready"
    break
  fi
  sleep 1
done

if ! curl -fsS "$SERVER_URL/health" >/dev/null 2>&1; then
  echo "[protocol-e2e-local] server did not become ready"
  echo "---- server log ----"
  cat "$SERVER_LOG"
  exit 1
fi

echo "[protocol-e2e-local] running SDK e2e smoke"
cd "$SDK_DIR"
pnpm install --frozen-lockfile || pnpm install
pnpm build
HERMES_E2E_BASE="$SERVER_URL" HERMES_E2E_TOKEN="$E2E_TOKEN" pnpm test:e2e

echo "[protocol-e2e-local] done"
