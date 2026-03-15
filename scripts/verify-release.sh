#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

release_version() {
  if [[ -n "${AXIOMNEXUS_RELEASE_VERSION:-}" ]]; then
    printf '%s\n' "$AXIOMNEXUS_RELEASE_VERSION"
    return
  fi

  if git rev-parse --short HEAD >/dev/null 2>&1; then
    git rev-parse --short HEAD
    return
  fi

  printf 'preview-local\n'
}

EVIDENCE_VERSION="$(release_version)"
EVIDENCE_DIR="${AXIOMNEXUS_RELEASE_EVIDENCE_DIR:-$ROOT_DIR/.axiomnexus/releases/$EVIDENCE_VERSION}"
VERIFY_LOG="${EVIDENCE_DIR%/}/verify-release.log"
export AXIOMNEXUS_RELEASE_EVIDENCE_DIR="$EVIDENCE_DIR"
export AXIOMNEXUS_SMOKE_LOG_PATH="${AXIOMNEXUS_SMOKE_LOG_PATH:-${EVIDENCE_DIR%/}/smoke-runtime.log}"
export AXIOMNEXUS_REPLAY_LOG_PATH="${AXIOMNEXUS_REPLAY_LOG_PATH:-${EVIDENCE_DIR%/}/replay.log}"
export AXIOMNEXUS_SMOKE_EXPORT_PATH="${AXIOMNEXUS_SMOKE_EXPORT_PATH:-${EVIDENCE_DIR%/}/store_snapshot.json}"

mkdir -p "$EVIDENCE_DIR"

if [[ -z "${AXIOMNEXUS_VERIFY_RELEASE_LOGGING:-}" ]]; then
  export AXIOMNEXUS_VERIFY_RELEASE_LOGGING=1
  exec > >(tee "$VERIFY_LOG") 2>&1
fi

echo "[1/4] scripts/verify-runtime.sh"
scripts/verify-runtime.sh

echo "[2/4] cargo test transition_intent_schema_gate_is_live_contract"
cargo test transition_intent_schema_gate_is_live_contract

echo "[3/4] cargo test execute_turn_output_schema_gate_is_live_contract"
cargo test execute_turn_output_schema_gate_is_live_contract

echo "[4/4] scripts/smoke-runtime.sh"
scripts/smoke-runtime.sh

echo "verify-release ok"
echo "release evidence dir: ${EVIDENCE_DIR#"$ROOT_DIR"/}"
