#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/axiomnexus-smoke.XXXXXX")"
DATA_DIR="$TMP_DIR/data"
PORT="$((4100 + ($$ % 400)))"
BASE_URL="http://127.0.0.1:${PORT}"
PROOF_FILE="smoke-runtime-proof.txt"
REPLIES_PATH="$TMP_DIR/scripted-replies.json"

if [[ -n "${AXIOMNEXUS_SMOKE_LOG_PATH:-}" ]]; then
  mkdir -p "$(dirname "$AXIOMNEXUS_SMOKE_LOG_PATH")"
  exec > >(tee "$AXIOMNEXUS_SMOKE_LOG_PATH") 2>&1
fi

cleanup() {
  stop_server
  rm -f "$ROOT_DIR/$PROOF_FILE"
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

http_json_any() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -s -X "$method" "$BASE_URL$path" \
      -H 'content-type: application/json' \
      --data "$body"
  else
    curl -s -X "$method" "$BASE_URL$path"
  fi
}

start_server() {
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
}

stop_server() {
  if [[ -n "${SERVER_PID:-}" ]]; then
    kill "${SERVER_PID}" >/dev/null 2>&1 || true
    wait "${SERVER_PID}" >/dev/null 2>&1 || true
    unset SERVER_PID
  fi
}

queued_run_id_for_work() {
  local work_id="$1"
  python3 -c 'import json,sys
work_id = sys.argv[1]
items = json.load(sys.stdin)["data"]["recent_runs"]
for item in items:
    if item["work_id"] == work_id and item["status"] == "queued":
        print(item["run_id"])
        break
else:
    raise SystemExit("queued run not found")' "$work_id"
}

lease_id_for_run() {
  python3 -c 'import sys
run_id = sys.argv[1]
hex_bytes = "".join(f"{byte:02x}" for byte in run_id.encode())
suffix = hex_bytes[-12:].rjust(12, "0")
print(f"00000000-0000-4000-8000-{suffix}")' "$1"
}

write_replies() {
  local path="$1"
  local work_id="$2"
  local agent_id="$3"
  local lease_id="$4"
  local expected_rev="$5"
  local runtime_session_id="$6"
  local summary="$7"
  local proof_file="$8"
  local resolved_obligation="$9"
  local invalid_first="${10}"

  python3 - <<'PY' "$path" "$work_id" "$agent_id" "$lease_id" "$expected_rev" "$runtime_session_id" "$summary" "$proof_file" "$resolved_obligation" "$invalid_first"
import json, sys

(
    path,
    work_id,
    agent_id,
    lease_id,
    expected_rev,
    runtime_session_id,
    summary,
    proof_file,
    resolved_obligation,
    invalid_first,
) = sys.argv[1:]

resolved = [resolved_obligation] if resolved_obligation else []
intent = {
    "work_id": work_id,
    "agent_id": agent_id,
    "lease_id": lease_id,
    "expected_rev": int(expected_rev),
    "kind": "complete",
    "patch": {
        "summary": summary,
        "resolved_obligations": resolved,
        "declared_risks": [],
    },
    "note": None,
    "proof_hints": [
        {"kind": "summary", "value": summary},
        {"kind": "file", "value": proof_file},
    ],
}
reply = {
    "handle": {"runtime_session_id": runtime_session_id},
    "raw_output": json.dumps(intent, separators=(",", ":")),
    "intent": intent,
    "usage": {
        "input_tokens": 21,
        "output_tokens": 13,
        "run_seconds": 1,
        "estimated_cost_cents": 2,
    },
    "invalid_session": False,
}
replies = [reply]
if invalid_first == "1":
    replies = [{
        "handle": {"runtime_session_id": "runtime-invalid-session"},
        "raw_output": "",
        "intent": intent,
        "usage": {
            "input_tokens": 0,
            "output_tokens": 0,
            "run_seconds": 0,
            "estimated_cost_cents": 0,
        },
        "invalid_session": True,
    }, reply]
with open(path, "w", encoding="utf-8") as fh:
    json.dump(replies, fh, separators=(",", ":"))
PY
}

mkdir -p "$DATA_DIR"

echo "[1/13] migrate"
migrate_output="$(run_cli cargo run --quiet -- migrate)"
printf '%s\n' "$migrate_output" | grep -q "axiomnexus migrate live"

echo "[2/13] doctor"
doctor_output="$(run_cli cargo run --quiet -- doctor)"
printf '%s\n' "$doctor_output" | grep -q "axiomnexus doctor live"

echo "[3/13] contract check"
contract_check_output="$(run_cli cargo run --quiet -- contract check)"
printf '%s\n' "$contract_check_output" | grep -q "axiomnexus contract check live"

echo "[4/13] serve"
start_server

echo "[5/13] onboarding and queue"
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

work_json="$(http_json POST /api/work "{\"company_id\":\"${company_id}\",\"parent_id\":null,\"kind\":\"task\",\"title\":\"Smoke Task\",\"body\":\"runtime smoke accepted path\",\"contract_set_id\":\"${contract_set_id}\"}")"
work_id="$(printf '%s' "$work_json" | json_get "work_id")"

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
before_rev="$(printf '%s' "$queued_detail" | python3 -c 'import json,sys
print(json.load(sys.stdin)["data"]["items"][0]["rev"])')"

echo "[6/13] wake queued work"
wake_body='{"latest_reason":"runtime smoke","obligation_delta":["runtime smoke follow up"]}'
wake_response="$(http_json POST "/api/work/${work_id}/wake" "$wake_body")"
printf '%s' "$wake_response" | grep -q '"queue_policy"'

agents_json="$(http_json GET /api/agents)"
run_id="$(printf '%s' "$agents_json" | queued_run_id_for_work "$work_id")"
lease_id="$(lease_id_for_run "$run_id")"

echo "[7/13] run once diagnostic accepted path"
printf 'smoke proof\n' >"$ROOT_DIR/$PROOF_FILE"
expected_rev="$((before_rev + 1))"
before_board="$(http_json GET /api/board)"
before_transition_count="$(printf '%s' "$before_board" | python3 -c 'import json,sys
print(len(json.load(sys.stdin)["data"]["recent_transition_records"]))')"
write_replies \
  "$REPLIES_PATH" \
  "$work_id" \
  "$agent_id" \
  "$lease_id" \
  "$expected_rev" \
  "runtime-smoke-1" \
  "runtime smoke complete" \
  "$PROOF_FILE" \
  "runtime smoke follow up" \
  "0"
stop_server
scheduler_output="$(AXIOMNEXUS_ALLOW_SCRIPTED_RUNTIME=1 AXIOMNEXUS_COCLAI_SCRIPT_PATH="$REPLIES_PATH" run_cli cargo run --quiet -- run once "$run_id")"
printf '%s\n' "$scheduler_output" | grep -q "axiomnexus run once live"
printf '%s\n' "$scheduler_output" | grep -q "run_id=${run_id}"
printf '%s\n' "$scheduler_output" | grep -q "repair_count=0"
start_server

echo "[8/13] direct transition record gate"
done_detail="$(http_json GET "/api/work/${work_id}")"
printf '%s' "$done_detail" | grep -q '"status":"done"'
after_rev="$(printf '%s' "$done_detail" | python3 -c 'import json,sys
print(json.load(sys.stdin)["data"]["items"][0]["rev"])')"
if [[ "$after_rev" -le "$before_rev" ]]; then
  echo "expected work rev to increase after runtime execute"
  exit 1
fi

board_json="$(http_json GET /api/board)"
printf '%s' "$board_json" | python3 -c 'import json,sys
before_count = int(sys.argv[1])
work_id = sys.argv[2]
data = json.load(sys.stdin)["data"]
if len(data["recent_transition_records"]) <= before_count:
    raise SystemExit("expected recent_transition_records to grow after accepted diagnostic run")
if not any(
    item["work_id"] == work_id
    and item["kind"] == "complete"
    and item["outcome"] in ("accepted", "override_accepted")
    for item in data["recent_transition_details"]
):
    raise SystemExit("expected accepted complete transition detail for work")
' "$before_transition_count" "$work_id"

run_detail="$(http_json GET "/api/runs/${run_id}")"
printf '%s' "$run_detail" | grep -q '"status":"completed"'
printf '%s' "$run_detail" | grep -q '"runtime_session_id":"runtime-smoke-1"'
printf '%s' "$run_detail" | python3 -c 'import json,sys
session = json.load(sys.stdin)["data"]["current_session"]
if session is None:
    raise SystemExit("expected current_session after accepted diagnostic run")
if session["runtime_session_id"] != sys.argv[1]:
    raise SystemExit("unexpected runtime_session_id in run detail")
' "runtime-smoke-1"

echo "[9/13] direct session and consumption gates"
printf '%s' "$board_json" | grep -q "\"work_id\":\"${work_id}\""
printf '%s' "$board_json" | grep -q '"kind":"complete"'
printf '%s' "$board_json" | grep -q '"summary":"Complete Accepted with next status Done"'
printf '%s' "$board_json" | grep -q '"total_turns":1'
printf '%s' "$board_json" | python3 -c 'import json,sys
summary = json.load(sys.stdin)["data"]["consumption_summary"]
if summary["total_turns"] < 1:
    raise SystemExit("expected total_turns >= 1")
if summary["total_input_tokens"] < 1:
    raise SystemExit("expected total_input_tokens >= 1")
if summary["total_output_tokens"] < 1:
    raise SystemExit("expected total_output_tokens >= 1")
if summary["total_estimated_cost_cents"] < 1:
    raise SystemExit("expected total_estimated_cost_cents >= 1")
'

agents_after_run="$(http_json GET /api/agents)"
printf '%s' "$agents_after_run" | python3 -c 'import json,sys
agent_id = sys.argv[1]
work_id = sys.argv[2]
runtime_session_id = sys.argv[3]
data = json.load(sys.stdin)["data"]
if not any(
    session["agent_id"] == agent_id
    and session["work_id"] == work_id
    and session["runtime_session_id"] == runtime_session_id
    for session in data["current_sessions"]
):
    raise SystemExit("expected persisted task_session in current_sessions")
agent_summary = next((item for item in data["consumption_by_agent"] if item["agent_id"] == agent_id), None)
if agent_summary is None:
    raise SystemExit("expected consumption summary for agent")
if agent_summary["total_turns"] < 1:
    raise SystemExit("expected agent total_turns >= 1")
if agent_summary["total_input_tokens"] < 1 or agent_summary["total_output_tokens"] < 1:
    raise SystemExit("expected agent token summary >= 1")
if agent_summary["total_estimated_cost_cents"] < 1:
    raise SystemExit("expected agent cost summary >= 1")
' "$agent_id" "$work_id" "runtime-smoke-1"

activity_json="$(http_json GET /api/activity)"
printf '%s' "$activity_json" | grep -q '"event_kind":"transition"'
printf '%s' "$activity_json" | grep -q '"summary":"Complete Accepted with next status Done"'

echo "[10/13] invalid-session repair path"
reopen_body="$(python3 - <<'PY' "$work_id" "$agent_id" "$after_rev"
import json, sys
print(json.dumps({
    "work_id": sys.argv[1],
    "agent_id": sys.argv[2],
    "lease_id": "board-lease",
    "expected_rev": int(sys.argv[3]),
    "kind": "reopen",
    "patch": {
        "summary": "",
        "resolved_obligations": [],
        "declared_risks": [],
    },
    "note": "reopen for repair smoke",
    "proof_hints": [{"kind": "summary", "value": "reopen for repair smoke"}],
}, separators=(",", ":")))
PY
)"
reopen_response="$(http_json POST "/api/work/${work_id}/reopen" "$reopen_body")"
printf '%s' "$reopen_response" | grep -q '"outcome":"accepted"\|"outcome":"override_accepted"'
reopened_detail="$(http_json GET "/api/work/${work_id}")"
printf '%s' "$reopened_detail" | grep -q '"status":"todo"'
reopened_rev="$(printf '%s' "$reopened_detail" | python3 -c 'import json,sys
print(json.load(sys.stdin)["data"]["items"][0]["rev"])')"

repair_wake='{"latest_reason":"repair smoke","obligation_delta":["repair smoke follow up"]}'
http_json POST "/api/work/${work_id}/wake" "$repair_wake" >/dev/null
repair_agents_json="$(http_json GET /api/agents)"
repair_run_id="$(printf '%s' "$repair_agents_json" | queued_run_id_for_work "$work_id")"
repair_lease_id="$(lease_id_for_run "$repair_run_id")"
repair_expected_rev="$((reopened_rev + 1))"
write_replies \
  "$REPLIES_PATH" \
  "$work_id" \
  "$agent_id" \
  "$repair_lease_id" \
  "$repair_expected_rev" \
  "runtime-smoke-2" \
  "runtime smoke repaired" \
  "$PROOF_FILE" \
  "repair smoke follow up" \
  "1"
stop_server
repair_output="$(AXIOMNEXUS_ALLOW_SCRIPTED_RUNTIME=1 AXIOMNEXUS_COCLAI_SCRIPT_PATH="$REPLIES_PATH" run_cli cargo run --quiet -- run once "$repair_run_id")"
printf '%s\n' "$repair_output" | grep -q "run_id=${repair_run_id}"
printf '%s\n' "$repair_output" | grep -q "session_reset_reason=runtime"
start_server

repair_run_detail="$(http_json GET "/api/runs/${repair_run_id}")"
printf '%s' "$repair_run_detail" | grep -q '"status":"completed"'
printf '%s' "$repair_run_detail" | grep -q '"runtime_session_id":"runtime-smoke-2"'

repair_board="$(http_json GET /api/board)"
printf '%s' "$repair_board" | grep -q '"total_turns":2'

echo "[11/13] rejected/conflict proxy gate"
bad_intent="$(python3 - <<'PY' "$work_id" "$agent_id"
import json, sys
print(json.dumps({
    "work_id": sys.argv[1],
    "agent_id": sys.argv[2],
    "lease_id": "missing-lease",
    "expected_rev": 0,
    "kind": "propose_progress",
    "patch": {
        "summary": "runtime failure smoke",
        "resolved_obligations": [],
        "declared_risks": [],
    },
    "note": None,
    "proof_hints": [{"kind": "summary", "value": "runtime failure smoke"}],
}, separators=(",", ":")))
PY
)"
bad_response="$(http_json_any POST "/api/work/${work_id}/intents" "$bad_intent")"
printf '%s' "$bad_response" | grep -q '"error"\|"outcome":"rejected"\|"outcome":"conflict"'

echo "[12/13] replay integrity proxy gate"
stop_server
replay_output="$(run_cli cargo run --quiet -- replay)"
printf '%s\n' "$replay_output" | grep -q "decision_path=transition_record"

echo "[13/13] smoke-runtime ok"
