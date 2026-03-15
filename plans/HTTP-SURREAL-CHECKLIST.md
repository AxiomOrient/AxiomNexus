# HTTP-SURREAL-CHECKLIST.md

목적:
- `src/adapter/http/**`
- `src/adapter/surreal/**`

를 빠르게 점검할 때 사용하는 grep 패턴과 판정표다.

이 문서는 **새 설계 문서가 아니라 현장 점검 체크리스트**다.

---

## 1. HTTP handler purity 점검

원칙:
- HTTP layer는 transport/DTO/route wiring만 담당
- business rule은 `src/app/**`, 상태 전이 규칙은 `src/kernel/**`
- handler가 직접 kernel rule을 집행하면 안 된다

## grep 패턴

```bash
rg -n "kernel::|ReasonCode|decide_transition|transition_record" src/adapter/http
```

### 통과
- 결과 없음
- 또는 test-only import만 있음
- 또는 dto/route 문자열 상수만 있음

### 경고
- handler가 `decide_transition` 직접 호출
- handler가 `ReasonCode` 분기 로직을 가짐
- handler가 `transition_record`를 직접 조립

### 조치
- app command/query로 위임
- handler는 parsing / response shaping만 남김

---

## 2. Surreal commit contract 점검

원칙:
- save-time stale rev 검증
- lease ownership 검증
- append-only TransitionRecord
- accepted일 때만 snapshot update
- replay 가능한 순서 유지

## grep 패턴

```bash
rg -n "fn commit_decision|expected_rev|lease_id|append.*record|transition_record" src/adapter/surreal
```

### 통과
- `commit_decision` 경로가 존재
- `expected_rev`를 사용
- `lease_id` 또는 lease ownership 검증이 존재
- record append 이후 snapshot update가 이어짐
- mismatch 시 에러를 반환

### 경고
- record append 없이 snapshot만 수정
- stale rev 검증 없음
- lease mismatch를 무시
- reject/conflict에도 accepted처럼 snapshot 갱신

### 조치
- save-time validation 보강
- record-first 의미론 복원
- accepted/non-accepted 분기 분명화

---

## 3. scripted/demo helper 오염 점검

원칙:
- smoke/test helper는 있어도 됨
- release/operator path를 오염시키면 안 됨

## grep 패턴

```bash
rg -n "seed_demo_state|ALLOW_SCRIPTED_RUNTIME|COCLAI_SCRIPT_PATH" src scripts
```

### 통과
- scripted helper가 smoke/test 전용으로만 사용
- README가 배포 환경에서 금지를 명시

### 경고
- 운영 코드에서 scripted path를 기본값처럼 사용
- release gate가 demo seed 없이는 통과 안 됨

### 조치
- env gate로 명확히 분리
- smoke/test 안으로 국한

---

## 4. operator path 문구 점검

원칙:
- `scheduler once` = 운영자 canonical path
- `run once <run_id>` = deterministic diagnostic path

## grep 패턴

```bash
rg -n "scheduler once|run once" README.md src/boot scripts
```

### 통과
- README / CLI / smoke step label이 같은 정의 사용

### 경고
- README는 운영용이라고 쓰고, 스크립트는 진단용으로 설명
- quick start와 CLI help가 다른 역할을 설명

### 조치
- README 기준 정의로 통일

---

## 5. release evidence 점검

원칙:
- verify/smoke/replay/export 산출물이 남아야 한다

## grep 패턴

```bash
rg -n "releases/|verify-release|smoke-runtime|replay" README.md scripts plans
```

### 통과
- evidence pack 저장 경로가 고정
- verify-release가 smoke/replay를 묶음

### 경고
- 스크립트는 있지만 산출물이 남지 않음
- 계획 문서와 실제 스크립트 경로가 다름

### 조치
- evidence export step 추가
- 경로를 `.axiomnexus/releases/<version>/`로 통일
