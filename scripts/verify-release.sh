#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

echo "[1/4] scripts/verify-runtime.sh"
scripts/verify-runtime.sh

echo "[2/4] cargo test transition_intent_schema_gate_is_live_contract"
cargo test transition_intent_schema_gate_is_live_contract

echo "[3/4] cargo test execute_turn_output_schema_gate_is_live_contract"
cargo test execute_turn_output_schema_gate_is_live_contract

echo "[4/4] scripts/smoke-runtime.sh"
scripts/smoke-runtime.sh

echo "verify-release ok"
