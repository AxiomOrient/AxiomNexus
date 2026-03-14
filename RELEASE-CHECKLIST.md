# Release Checklist

## 목적

preview release 전 마지막 정합성 점검은 문서 설명과 실제 증거가 같아야 한다.

---

## 1. 실행 경로 용어

- canonical operator path: `cargo run -- scheduler once`
- deterministic diagnostic path: `cargo run -- run once <run_id>`
- release smoke는 queued run을 만들고, diagnostic path로 accepted / repair / reject 흐름을 직접 검증한다.

이 셋이 `README.md`, `docs/04-API-SURFACE.md`, `docs/05-QUALITY-GATES.md`, `scripts/smoke-runtime.sh`와 같은 말을 해야 한다.

---

## 2. Ship-Now Gate

```bash
scripts/verify-release.sh
```

필수 통과 조건:

1. `scripts/verify-runtime.sh`
2. `cargo test transition_intent_schema_gate_is_live_contract`
3. `cargo test execute_turn_output_schema_gate_is_live_contract`
4. `scripts/smoke-runtime.sh`

---

## 3. Runtime Smoke Evidence

smoke가 직접 확인해야 하는 항목:

1. accepted diagnostic run 뒤 `TransitionRecord`가 read model에 append 된다.
2. `GET /api/runs/{id}`와 `GET /api/agents`에서 `task_session` persistence가 보인다.
3. board / agents read model에서 consumption turn / token / cost summary가 증가한다.
4. invalid-session repair path가 session reset 후 completed로 끝난다.
5. rejected / conflict path는 proxy gate로 남기되, accepted path 증거는 direct gate로 확인한다.
6. `cargo run -- replay`가 `decision_path=transition_record`로 끝난다.

---

## 4. Evidence Pack

권장 명령:

```bash
scripts/export-release-evidence.sh v0.1.0-preview.1 axiomnexus-v0.1.0-preview.1 preview
```

산출물 예시:

- `.axiomnexus/releases/v0.1.0-preview.1/verify-release.log`
- `.axiomnexus/releases/v0.1.0-preview.1/smoke-runtime.log`
- `.axiomnexus/releases/v0.1.0-preview.1/replay.log`
- `.axiomnexus/releases/v0.1.0-preview.1/store_snapshot.json`
- `.axiomnexus/releases/v0.1.0-preview.1/release-notes.md`

---

## 5. Rollback

1. 직전 tag를 기록한다.
2. evidence pack 안 `store_snapshot.json` 경로를 release notes에 남긴다.
3. 복구가 필요하면 snapshot import와 replay를 다시 실행해 mismatch 0을 확인한다.
