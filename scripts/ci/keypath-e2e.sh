#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT_DIR"

echo "[keypath-e2e] agent loop"
cargo test -p hermes-agent e2e_agent_loop_tool_call_then_final_reply -- --nocapture

echo "[keypath-e2e] gateway routing"
cargo test -p hermes-gateway e2e_gateway_routes_message_and_replies -- --nocapture

echo "[keypath-e2e] http command bridge"
cargo test -p hermes-dashboard command_help_runs_through_gateway -- --nocapture

echo "[keypath-e2e] cli model/status"
cargo test -p hermes-cli e2e_cli_model_command_prints_current_model -- --nocapture

echo "[keypath-e2e] gateway process lifecycle"
cargo test -p hermes-cli e2e_gateway_subprocess_lifecycle_start_status_sigint -- --nocapture --test-threads=1

echo "[keypath-e2e] eval runner smoke"
HERMES_EVAL_MAX_TASKS=1 cargo run -p hermes-eval --bin hermes-bench-smoke --quiet >/dev/null

echo "[keypath-e2e] ok"
