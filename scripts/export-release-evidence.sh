#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

usage() {
  cat <<'EOF'
usage: scripts/export-release-evidence.sh <version> <tag> [preview|stable]
EOF
}

if [[ $# -lt 2 || $# -gt 3 ]]; then
  usage >&2
  exit 1
fi

VERSION="$1"
TAG="$2"
RELEASE_TYPE="${3:-preview}"
EVIDENCE_DIR="$ROOT_DIR/.axiomnexus/releases/$VERSION"
VERIFY_LOG="$EVIDENCE_DIR/verify-release.log"
REPLAY_LOG="$EVIDENCE_DIR/replay.log"
SNAPSHOT_PATH="$EVIDENCE_DIR/store_snapshot.json"
RELEASE_NOTES_PATH="$EVIDENCE_DIR/release-notes.md"

mkdir -p "$EVIDENCE_DIR"

export AXIOMNEXUS_RELEASE_EVIDENCE_DIR="$EVIDENCE_DIR"
export AXIOMNEXUS_RELEASE_VERSION="$VERSION"

scripts/verify-release.sh

cat >"$RELEASE_NOTES_PATH" <<EOF
# Release Notes

## Release

- version: $VERSION
- tag: $TAG
- date: $(date +%F)
- type: $RELEASE_TYPE

## Summary

- ship-now gate result: pass
- runtime smoke result: pass
- replay result: pass
- canonical operator path check: \`scheduler once\`
- diagnostic path check: \`run once <run_id>\`

## Evidence

- evidence dir: .axiomnexus/releases/$VERSION/
- smoke log: .axiomnexus/releases/$VERSION/smoke-runtime.log
- verify log: .axiomnexus/releases/$VERSION/verify-release.log
- replay log: .axiomnexus/releases/$VERSION/replay.log
- export snapshot: .axiomnexus/releases/$VERSION/store_snapshot.json

## Known limitations

- embedded SurrealKV preview runtime

## Rollback

- previous tag:
- restore snapshot: .axiomnexus/releases/$VERSION/store_snapshot.json
- compatibility notes:
EOF

printf 'release evidence exported: %s\n' "$EVIDENCE_DIR"
