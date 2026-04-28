#!/usr/bin/env bash
set -euo pipefail

# Cloud Agent API end-to-end smoke script.
#
# Required:
#   HERMES_CLOUD_BEARER   JWT bearer token for cloud routes
#   REPO_URL              git repository URL for cloud agent session
#
# Optional:
#   HERMES_BASE_URL       default: http://127.0.0.1:3000
#   BRANCH                default: main
#   MODEL                 default: (omit)
#   PROMPT                default: "请读取仓库并回复当前分支名"
#   TARGET_BRANCH         default: BRANCH
#   CLEANUP               default: 1 (delete agent at end)

if ! command -v curl >/dev/null 2>&1; then
  echo "error: curl is required" >&2
  exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
  echo "error: jq is required" >&2
  exit 1
fi

BASE_URL="${HERMES_BASE_URL:-http://127.0.0.1:3000}"
TOKEN="${HERMES_CLOUD_BEARER:-}"
REPO_URL="${REPO_URL:-}"
BRANCH="${BRANCH:-main}"
PROMPT="${PROMPT:-请读取仓库并回复当前分支名}"
TARGET_BRANCH="${TARGET_BRANCH:-$BRANCH}"
CLEANUP="${CLEANUP:-1}"

if [[ -z "$TOKEN" ]]; then
  echo "error: HERMES_CLOUD_BEARER is required" >&2
  exit 1
fi
if [[ -z "$REPO_URL" ]]; then
  echo "error: REPO_URL is required" >&2
  exit 1
fi

auth_headers=(
  -H "Authorization: Bearer $TOKEN"
  -H "Content-Type: application/json"
)

echo "==> 1) health check"
curl -sS "$BASE_URL/health" | jq .

echo "==> 2) verify tenant profile"
curl -sS "${auth_headers[@]}" "$BASE_URL/api/v1/tenant/profile" | jq .

echo "==> 3) create cloud agent session"
create_payload="$(jq -n \
  --arg repo "$REPO_URL" \
  --arg branch "$BRANCH" \
  --arg model "${MODEL:-}" \
  '{
    repo_url: $repo,
    branch: $branch,
    startup_commands: []
  } + (if $model == "" then {} else {model: $model} end)')"
create_resp="$(curl -sS -X POST "${auth_headers[@]}" \
  -d "$create_payload" \
  "$BASE_URL/api/v1/agents")"
echo "$create_resp" | jq .
AGENT_ID="$(echo "$create_resp" | jq -r '.id // empty')"
if [[ -z "$AGENT_ID" ]]; then
  echo "error: create agent did not return id" >&2
  exit 1
fi
echo "created agent id: $AGENT_ID"

echo "==> 4) send message"
msg_payload="$(jq -n --arg text "$PROMPT" '{text: $text}')"
msg_resp="$(curl -sS -X POST "${auth_headers[@]}" \
  -d "$msg_payload" \
  "$BASE_URL/api/v1/agents/$AGENT_ID/messages")"
echo "$msg_resp" | jq .

echo "==> 5) list messages (control-plane persisted)"
messages_resp="$(curl -sS "${auth_headers[@]}" \
  "$BASE_URL/api/v1/agents/$AGENT_ID/messages")"
echo "$messages_resp" | jq .
message_count="$(echo "$messages_resp" | jq '.messages | length')"
if [[ "$message_count" -lt 2 ]]; then
  echo "error: expected at least 2 persisted messages, got $message_count" >&2
  exit 1
fi

echo "==> 6) patch git policy (commit on, push off)"
policy_payload="$(jq -n \
  --arg target "$TARGET_BRANCH" \
  '{
    auto_commit_enabled: true,
    auto_push_enabled: false,
    target_branch: $target,
    protected_branches: ["main", "master"]
  }')"
curl -sS -X PATCH "${auth_headers[@]}" \
  -d "$policy_payload" \
  "$BASE_URL/api/v1/agents/$AGENT_ID/git-policy" | jq .

echo "==> 7) verify agent detail"
curl -sS "${auth_headers[@]}" "$BASE_URL/api/v1/agents/$AGENT_ID" | jq .

if [[ "$CLEANUP" == "1" ]]; then
  echo "==> 8) cleanup agent"
  curl -sS -X DELETE "${auth_headers[@]}" \
    "$BASE_URL/api/v1/agents/$AGENT_ID" | jq .
fi

echo "Cloud Agent E2E smoke finished."
