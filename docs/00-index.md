# AxiomNexus 문서 인덱스

canonical reader path는 두 곳으로 고정한다.

1. root `README.md`
2. `docs/00-index.md`

먼저 읽을 문서:

1. `00-DESIGN-REVIEW.md` — 왜 final package를 이렇게 정리했는지
2. `01-FINAL-TARGET.md` — 최종 도착지와 범위
3. `02-BLUEPRINT.md` — 정적 구조와 제어 흐름
4. `03-DOMAIN-AND-INVARIANTS.md` — 핵심 모델과 불변식
5. `04-API-SURFACE.md` — CLI / HTTP / SSE 표면
6. `05-QUALITY-GATES.md` — 품질 게이트와 검증 전략
7. `spec/STOREPORT-SEMANTIC-CONTRACT.md` — 저장소 의미론 기준 계약
8. `spec/RUNTIMEPORT-EXECUTE-TURN-SPEC.md` — runtime turn 최종 계약
9. `RELEASE-NOTES-TEMPLATE.md` — release note 기록 형식

정리 원칙:

- 새 독자 입구는 root `README.md`와 `docs/00-index.md`만 쓴다.
- `docs/`는 제품/계약 문서 표면이다.
- `docs/spec/`는 바뀌면 안 되는 계약 문서다.
- `docs/adr/`는 구조 결정을 기록한다.
- `README.md`, `docs/`, `samples/`는 같은 canonical 구조를 말해야 한다.

역사 참고 문서:

- `01-system-design.md` — 초기 제품 경계 요약. 현재 canonical entry는 아님
- `05-target-architecture.md` — 초기 목표 구조 요약. 현재 canonical entry는 아님

이 두 문서는 삭제 대상은 아니지만, 새 작업의 시작점으로 쓰지 않는다.
