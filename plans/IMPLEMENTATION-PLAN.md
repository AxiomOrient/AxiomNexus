# IMPLEMENTATION-PLAN

## 현재 상태

preview release-grade consistency 마감 작업은 완료했다.
앞으로의 기본 문서는 이 폴더를 기준으로 이어간다.

---

## Done Ledger

### 1. Operator path 정리
- `scheduler once`를 canonical operator path로 고정
- `run once <run_id>`를 deterministic diagnostic path로 고정
- `README.md`, `docs/04-API-SURFACE.md`, `RELEASE-CHECKLIST.md`, `scripts/smoke-runtime.sh` 용어 정렬

### 2. Direct evidence gate 강화
- smoke에서 accepted path 뒤 direct transition record assertion 추가
- smoke에서 `task_session` persistence direct assertion 추가
- smoke에서 board / agent consumption summary direct assertion 추가
- replay는 integrity proxy gate로 분리

### 3. Release evidence automation
- `scripts/export-release-evidence.sh` 추가
- `.axiomnexus/releases/<version>/` 구조 고정
- `scripts/verify-release.sh`가 smoke 로그 산출과 연결
- `docs/RELEASE-NOTES-TEMPLATE.md`를 실제 evidence 경로 형식에 맞춤

---

## Next Ledger

### Preview 운영
- release candidate마다 `scripts/export-release-evidence.sh <version> <tag> [type]` 실행
- evidence pack을 release note와 함께 보관

### Stable kickoff
- PostgreSQL adapter 설계에서 구현 단계로 이동
- dual-store conformance 실행체를 추가
- benchmark baseline artifact 저장 경로를 실제로 채움
- observability span/event를 코드와 문서에 연결

---

## Open Decisions

1. evidence pack을 CI가 아니라 로컬 release 명령으로만 유지할지
2. stable 진입 전에 benchmark artifact 저장 형식을 JSON으로 고정할지
3. observability audit를 smoke 이후 별도 스크립트로 둘지
