# AxiomNexus Release Readiness Package

이 패키지는 `dev@d975380` 기준 저장소를 전제로,
이전 패키지의 수정이 **이미 반영되었다고 가정한 뒤** 남은 출시 작업만 추린 핵심 문서 묶음이다.

## 판단 요약

- **이전 핵심 수정 중 상당수는 이미 반영되었다.**
  - root package = `axiomnexus`
  - `WorkspacePort` 제거
  - `RuntimePort::execute_turn` + `observations`
  - Surreal-first
  - `verify-runtime.sh` / `verify-release.sh` / `smoke-runtime.sh`
- **그래도 지금 바로 최종/안정판 출시를 권하지는 않는다.**
- 남은 일은 “설계 변경”이 아니라 **릴리스 정합성 마감**이다.

## 포함 문서

- `RELEASE-READINESS.md` — 출시 가능 여부와 blocker
- `ROADMAP.md` — 출시 전 남은 단계별 로드맵
- `IMPLEMENTATION-PLAN.md` — 각 단계별 구체 작업 계획
- `TASKS.md` — 태스크 ledger
- `COMMIT-PLAN.md` — 12개 커밋 단위 실행안
- `../RELEASE-CHECKLIST.md` — 최종 출시 체크리스트

## 권장 해석

- 내부 dogfood / alpha: **가까움**
- public stable / final: **아직 아님**
