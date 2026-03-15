# 청사진

## 상위 구조

```text
src/
  boot/
  model/
  kernel/
  app/
  port/
  adapter/
```

## 각 경계의 책임

### `model`
- 타입과 데이터 계약만 둔다
- 외부 I/O를 두지 않는다

### `kernel`
- 상태 전이 규칙과 순수 계산만 둔다
- 저장, 파일, 프로세스 실행을 두지 않는다

### `app`
- use-case 흐름을 조립한다
- context load, evidence assembly, kernel 호출, commit 호출을 맡는다

### `port`
- 외부 경계 trait만 둔다

### `adapter`
- store, runtime, HTTP, SSE 구현만 둔다

### `boot`
- 설정, wiring, CLI 진입점만 둔다

## 허용 방향

```text
boot -> app -> kernel -> model
app  -> port
adapter -> port
adapter -> model
boot -> adapter
```

## 금지 방향

- `kernel -> app`
- `kernel -> adapter`
- `model -> adapter`

## 핵심 쓰기 경로

```text
load_context
  -> collect evidence
  -> kernel::decide_transition
  -> store.commit_decision
```

이 경로 밖에서 authoritative 상태를 바꾸지 않는다.

## runtime turn 흐름

```text
load_runtime_turn
  -> resume / execute runtime
  -> collect observations
  -> build evidence
  -> decide
  -> commit
```

핵심은 runtime이 판정하지 않는다는 점이다.
runtime은 관측을 반환하고, kernel이 그 관측을 해석한다.

## 문서 경계

- 구조는 이 문서에 둔다
- 타입 규칙은 `03-DOMAIN-AND-INVARIANTS.md`에 둔다
- 운영 표면은 `04-API-SURFACE.md`에 둔다
- 저장 의미 규칙은 `spec/STOREPORT-SEMANTIC-CONTRACT.md`에 둔다
