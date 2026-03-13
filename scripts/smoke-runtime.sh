#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/axiomnexus-smoke.XXXXXX")"
DATA_DIR="$TMP_DIR/data"
PORT="$((4100 + ($$ % 400)))"
BASE_URL="http://127.0.0.1:${PORT}"

cleanup() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
  fi
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

run_cli() {
  local output
  output="$(AXIOMNEXUS_DATA_DIR="$DATA_DIR" "$@" 2>&1)"
  printf '%s\n' "$output"
}

json_get() {
  local expr="$1"
  python3 -c 'import json,sys
expr = sys.argv[1]
value = json.load(sys.stdin)
for part in expr.split("."):
    value = value[int(part)] if part.isdigit() else value[part]
if isinstance(value, str):
    print(value)
else:
    print(json.dumps(value, separators=(",", ":")))' "$expr"
}

http_json() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -sf -X "$method" "$BASE_URL$path" \
      -H 'content-type: application/json' \
      --data "$body"
  else
    curl -sf -X "$method" "$BASE_URL$path"
  fi
}

mkdir -p "$DATA_DIR"

echo "[1/8] migrate"
migrate_output="$(run_cli cargo run --quiet -- migrate)"
printf '%s\n' "$migrate_output" | grep -q "axiomnexus migrate live"

echo "[2/8] doctor"
doctor_output="$(run_cli cargo run --quiet -- doctor)"
printf '%s\n' "$doctor_output" | grep -q "axiomnexus doctor live"

echo "[3/8] contract check"
contract_check_output="$(run_cli cargo run --quiet -- contract check)"
printf '%s\n' "$contract_check_output" | grep -q "axiomnexus contract check live"

echo "[4/8] serve"
AXIOMNEXUS_DATA_DIR="$DATA_DIR" \
AXIOMNEXUS_HTTP_ADDR="127.0.0.1:${PORT}" \
  cargo run --quiet -- serve >"$TMP_DIR/serve.log" 2>&1 &
SERVER_PID=$!

for _ in $(seq 1 50); do
  if curl -sf "$BASE_URL/api/board" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done
curl -sf "$BASE_URL/api/board" >/dev/null

echo "[5/8] onboarding"
contracts_json="$(http_json GET /api/contracts/active)"
rules_json="$(printf '%s' "$contracts_json" | json_get "data.rules")"

company_json="$(http_json POST /api/companies '{"name":"Smoke Labs","description":"runtime smoke"}')"
company_id="$(printf '%s' "$company_json" | json_get "company_id")"

contract_json="$(http_json POST /api/contracts "$(python3 - <<'PY' "$company_id" "$rules_json"
import json, sys
company_id = sys.argv[1]
rules = json.loads(sys.argv[2])
print(json.dumps({
    "company_id": company_id,
    "name": "smoke-contract",
    "rules": rules,
}, separators=(",", ":")))
PY
)")"
revision="$(printf '%s' "$contract_json" | json_get "revision")"

http_json POST "/api/contracts/${revision}/activate" "{\"company_id\":\"${company_id}\"}" >/dev/null
agent_json="$(http_json POST /api/agents "{\"company_id\":\"${company_id}\",\"name\":\"Smoke Agent\",\"role\":\"implementer\"}")"
agent_id="$(printf '%s' "$agent_json" | json_get "agent_id")"

companies_json="$(http_json GET /api/companies)"
contract_set_id="$(printf '%s' "$companies_json" | python3 -c 'import json,sys
company_id = sys.argv[1]
items = json.load(sys.stdin)["data"]["items"]
for item in items:
    if item["company_id"] == company_id:
        print(item["active_contract_set_id"])
        break
' "$company_id")"

work_json="$(http_json POST /api/work "{\"company_id\":\"${company_id}\",\"parent_id\":null,\"kind\":\"task\",\"title\":\"Smoke Task\",\"body\":\"queue via smoke\",\"contract_set_id\":\"${contract_set_id}\"}")"
work_id="$(printf '%s' "$work_json" | json_get "work_id")"

echo "[6/8] queue"
queue_body="$(python3 - <<'PY' "$work_id" "$agent_id"
import json, sys
print(json.dumps({
    "work_id": sys.argv[1],
    "agent_id": sys.argv[2],
    "lease_id": "board-lease",
    "expected_rev": 0,
    "kind": "queue",
    "patch": {
        "summary": "",
        "resolved_obligations": [],
        "declared_risks": [],
    },
    "note": "board action",
    "proof_hints": [{"kind": "summary", "value": "board action"}],
}, separators=(",", ":")))
PY
)"
queue_response="$(http_json POST "/api/work/${work_id}/queue" "$queue_body")"
printf '%s' "$queue_response" | grep -q '"outcome":"accepted"\|"outcome":"override_accepted"'
queued_detail="$(http_json GET "/api/work/${work_id}")"
printf '%s' "$queued_detail" | grep -q '"status":"todo"'
printf '%s' "$queued_detail" | grep -q '"rev":1'

echo "[7/8] runtime intent turn"
intent_body="$(python3 - <<'PY' "$work_id" "$agent_id"
import json, sys
print(json.dumps({
    "work_id": sys.argv[1],
    "agent_id": sys.argv[2],
    "lease_id": "missing-lease",
    "expected_rev": 1,
    "kind": "propose_progress",
    "patch": {
        "summary": "smoke runtime turn",
        "resolved_obligations": [],
        "declared_risks": [],
    },
    "note": None,
    "proof_hints": [{"kind": "summary", "value": "smoke runtime turn"}],
}, separators=(",", ":")))
PY
)"
intent_response="$(http_json POST "/api/work/${work_id}/intents" "$intent_body")"
printf '%s' "$intent_response" | grep -q '"outcome":"rejected"\|"outcome":"conflict"'

kill "${SERVER_PID}" >/dev/null 2>&1 || true
wait "${SERVER_PID}" >/dev/null 2>&1 || true
unset SERVER_PID

echo "[8/8] replay"
replay_output="$(run_cli cargo run --quiet -- replay)"
printf '%s\n' "$replay_output" | grep -q "decision_path=transition_record"

echo "smoke-runtime ok"
