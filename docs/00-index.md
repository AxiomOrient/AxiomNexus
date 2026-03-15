# AxiomNexus 문서 맵

문서 입구는 두 곳만 쓴다.

1. `README.md`
2. `docs/00-index.md`

가장 먼저 볼 운영 흐름은 `README.md`의 `Preview workflow`다.

## 1. 제품 정의

1. `01-FINAL-TARGET.md`
   현재 제품이 무엇인지, 무엇을 포함하고 무엇을 제외하는지 정리한다.
2. `03-DOMAIN-AND-INVARIANTS.md`
   핵심 데이터와 반드시 지켜야 할 규칙만 모아 둔다.

## 2. 구조와 표면

3. `02-BLUEPRINT.md`
   코드 경계, 의존 방향, 쓰기 경로를 설명한다.
4. `04-API-SURFACE.md`
   CLI, HTTP, SSE의 살아 있는 운영 표면만 정리한다.

## 3. 계약 문서

5. `spec/STOREPORT-SEMANTIC-CONTRACT.md`
   저장소 의미 규칙의 기준 문서다.
6. `spec/RUNTIMEPORT-EXECUTE-TURN-SPEC.md`
   runtime turn 입력과 출력 계약의 기준 문서다.
7. `spec/CONFORMANCE-SUITE.md`
   저장소 adapter가 같은 의미를 지키는지 검증하는 목록이다.

## 4. 운영과 변경 이력

8. `05-QUALITY-GATES.md`
   개발, 릴리스, 회귀 검증 순서를 정리한다.
9. `adr/`
   이미 확정된 구조 결정을 짧게 기록한다.

## 문서 원칙

- 문서는 한 주제당 한 파일만 기준으로 둔다.
- 역사 설명은 별도 참고 문서로 늘리지 않는다.
- 현재 제품 표면과 직접 연결되지 않는 초안, 회고, 중복 설명은 남기지 않는다.
- `README.md`, `docs/`, `samples/`는 같은 구조를 말해야 한다.
