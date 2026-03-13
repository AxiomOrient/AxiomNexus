# AxiomNexus 문서 인덱스

현재 canonical 문서는 runtime/control-plane 제품 표면과 final target을 함께 설명합니다.

먼저 읽을 문서:

1. `00-DESIGN-REVIEW.md` — 왜 final package를 이렇게 정리했는지
2. `01-FINAL-TARGET.md` — 최종 도착지와 범위
3. `02-BLUEPRINT.md` — 정적 구조와 제어 흐름
4. `03-DOMAIN-AND-INVARIANTS.md` — 핵심 모델과 불변식
5. `04-API-SURFACE.md` — CLI / HTTP / SSE 표면
6. `05-QUALITY-GATES.md` — 품질 게이트와 검증 전략
7. `spec/STOREPORT-SEMANTIC-CONTRACT.md` — 저장소 의미론 기준 계약
8. `spec/RUNTIMEPORT-EXECUTE-TURN-SPEC.md` — runtime turn 최종 계약
9. `../plans/IMPLEMENTATION-PLAN.md` — 현재 미션 실행 계획
10. `../plans/TASKS.md` — 실행 ledger

정리 원칙:

- `docs/`가 공식 읽기 표면이다.
- `docs/spec/`는 바뀌면 안 되는 계약 문서다.
- `docs/adr/`는 구조 결정을 기록한다.
- `README.md`, `docs/`, `plans/`, `samples/`가 같은 canonical 구조를 말해야 한다.

현재 상태를 설명하는 참고 문서:

- `01-system-design.md` — 현재 제품 경계와 핵심 데이터 모델
- `05-target-architecture.md` — 현재 저장소 기준 목표 구조 요약
