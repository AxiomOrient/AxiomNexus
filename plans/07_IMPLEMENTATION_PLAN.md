# 07_IMPLEMENTATION_PLAN.md

# AxiomNexus 구현 계획
기준선:
- branch: `dev`
- 포함 커밋: `12f0ab2`
- 이번 계획은 **PostgreSQL 작업 제외**, **Surreal-first 유지**, **coclai 하나만 runtime 유지**를 전제로 한다.

## 0. 목적

이번 단계의 목적은 새 기능을 크게 늘리는 것이 아니다.

목표는 하나다.

> **AxiomNexus를 “AI 소프트웨어 팀의 업무 control plane”으로 실제로 쓸 수 있는 preview-grade 제품 상태까지 정확하게 닫는다.**

즉 아래 둘 다 만족해야 한다.

1. 운영자가 실제로 `company → contract → agent → work → queue → scheduler once → activity/replay` 흐름으로 사용할 수 있어야 한다.
2. 그 흐름의 상태 전이가 `Intent -> Decide -> Commit` 계약, append-only `TransitionRecord`, replay 정합성으로 설명 가능해야 한다.

---

## 1. 이번 버전의 고정 원칙

## 유지
- single crate
- `RuntimePort::execute_turn`
- `StorePort` semantic contract
- append-only `TransitionRecord`
- SurrealKV live store
- coclai runtime 하나
- operator path = `scheduler once`
- diagnostic path = `run once <run_id>`

## 금지
- PostgreSQL adapter 시작
- multi-runtime 일반화
- goals / budgets / org chart 같은 broad company OS 확장
- HTTP handler에서 business rule 추가
- `src/model`, `src/kernel` 밖으로 상태 전이 규칙 분산
- `TransitionRecord`를 수정 가능한 log처럼 다루기

---

## 2. 이번 단계에서 닫아야 할 제품 표면

이번 버전에서 제품으로 보장할 표면은 아래로 제한한다.

### 운영 표면
- company
- contract
- agent
- work
- run
- activity
- replay
- export / import

### 실행 표면
- `cargo run -- serve`
- `cargo run -- scheduler once`
- `cargo run -- run once <run_id>`
- `scripts/verify-runtime.sh`
- `scripts/verify-release.sh`
- `scripts/smoke-runtime.sh`

### 핵심 커널
- `TransitionIntent`
- `decide_transition`
- `commit_decision`
- `TransitionRecord`
- replay

이 범위를 넘는 확장은 전부 다음 버전 이후로 미룬다.

---

## 3. 최우선 전략

지금 가장 안전하고 단순한 전략은 **release-grade consistency 강화**다.

즉, 새 abstraction이나 새 기능보다 아래 4가지를 먼저 닫는다.

1. 제품 정체성 문구 고정
2. operator path 문구/운영 방식 통일
3. smoke / verify가 핵심 증거를 직접 검증하도록 강화
4. release evidence pack을 운영 절차로 고정

왜냐하면:
- 아키텍처 큰 축은 이미 들어와 있다.
- 지금 리스크는 “설계 미완성”보다 “운영자가 실제로 어떻게 써야 하는지 모호한 것”에 가깝다.
- 이 단계에서 구조를 다시 흔들면 얻는 것보다 잃는 것이 크다.

---

## 4. 단계별 실행 전략

## Phase 1 — 제품 정체성 잠금

### 목표
README와 plans가 이 프로젝트를 정확히 같은 말로 설명하게 만든다.

### 핵심 문장
AxiomNexus는:

> **AI 소프트웨어 팀의 work/run 상태를 운영하고, 각 상태 전이를 계약과 증거로 판정·기록하는 control plane**

이다.

### 변경 파일
- `README.md`
- `plans/07_IMPLEMENTATION_PLAN.md`
- `plans/TASKS.md`

### 작업
1. README 첫 문단에서 “회사 운영 OS 아님”, “단순 코드 변경 승인기만도 아님”을 분명히 한다.
2. quick start와 preview workflow를 실제 operator 흐름 기준으로 다시 확인한다.
3. plans 문서도 같은 제품 정의를 사용한다.
4. “무엇을 하지 않는가”를 짧고 분명하게 둔다.

### 완료 조건
- README와 plans를 읽은 사람이 같은 제품을 상상한다.
- scope drift가 줄어든다.

### 검증 명령
```bash
rg -n "control plane|회사 운영 OS|scheduler once|run once" README.md plans
```

### 중단 조건
- 제품 정의를 넓혀 goals/budget/org chart 수준까지 다시 확장하려는 논의가 나오면 중단
- 반대로 단순 “코드 변경 승인기”로만 축소하려는 논의가 나오면 중단

---

## Phase 2 — operator path 통일

### 목표
운영 경로와 진단 경로를 문서/CLI/스크립트에서 같은 언어로 설명한다.

### 정책
- `scheduler once` = canonical operator path
- `run once <run_id>` = deterministic diagnostic path

### 변경 파일
- `README.md`
- `scripts/smoke-runtime.sh`
- 필요 시 `src/boot/cli.rs`
- 필요 시 `docs/04-API-SURFACE.md` (존재한다면)

### 작업
1. README quick start에 두 명령의 책임을 명확히 적는다.
2. smoke script 로그가 실제로 어느 경로를 검증하는지 step label을 정리한다.
3. CLI help가 모호하면 문구만 조정한다.
4. “운영자는 `scheduler once`를 사용한다”를 제품 기준선으로 고정한다.

### 완료 조건
- README / CLI / smoke log가 같은 언어를 쓴다.
- 사용자가 언제 `scheduler once`, 언제 `run once`를 써야 하는지 헷갈리지 않는다.

### 검증 명령
```bash
rg -n "scheduler once|run once" README.md src/boot scripts/smoke-runtime.sh
```

### 중단 조건
- operator path를 다시 여러 개로 늘리려는 시도가 나오면 중단

---

## Phase 3 — release gate 직접 증거 강화

### 목표
checklist에 적힌 핵심 항목을 smoke/verify가 직접 확인하게 만든다.

### 직접 확인해야 할 항목
- accepted transition
- `TransitionRecord` append
- `WorkSnapshot.rev` 증가
- run completed
- `task_session` 저장/갱신
- consumption 기록
- replay pass

### 변경 파일
- `scripts/smoke-runtime.sh`
- `scripts/verify-release.sh`
- 필요 시 `src/adapter/http/**` read model output
- 필요 시 `src/adapter/surreal/**` query support

### 작업
1. accepted transition을 smoke output에서 명시 assertion으로 고정한다.
2. latest transition record 또는 record count를 직접 확인한다.
3. before/after revision 비교를 명시적으로 남긴다.
4. task session 존재/갱신을 조회해 검증한다.
5. consumption summary 또는 event summary를 직접 검증한다.
6. replay success를 integrity gate로 분명히 유지한다.

### 완료 조건
- release checklist가 요구하는 핵심 항목과 smoke 검증 항목이 1:1로 맞는다.
- preview release를 “감으로” 판단하지 않게 된다.

### 검증 명령
```bash
scripts/smoke-runtime.sh
scripts/verify-release.sh
```

### 중단 조건
- smoke가 점점 broad integration monster가 되기 시작하면 중단
- preview 범위를 넘는 성능/scale 검증까지 한 번에 넣으려 하면 중단

---

## Phase 4 — HTTP purity / Surreal commit contract 점검

### 목표
문서 계약의 핵심 금지 규칙을 실제 코드로 다시 잠근다.

### 왜 지금 필요한가
제품 수준에서 중요한 것은 “잘 돌아간다”만이 아니라
“잘못된 위치에 규칙이 들어가 있지 않다”는 것이다.

### 변경 파일
- `src/adapter/http/**`
- `src/adapter/surreal/**`
- 필요 시 관련 테스트

### 작업
1. HTTP handler에서 kernel/business rule 직접 호출이 없는지 확인한다.
2. handler는 app command/query orchestration만 하게 유지한다.
3. Surreal `commit_decision`가 save-time validation을 수행하는지 확인한다.
4. stale rev / lease mismatch / record append / snapshot update ordering을 확인한다.
5. 필요 시 테스트 이름과 메시지만 보강한다.

### 완료 조건
- transport에는 transport만 남는다.
- store adapter는 문서 계약을 어기지 않는다.

### 검증 명령
```bash
rg -n "kernel::|ReasonCode|decide_transition|transition_record" src/adapter/http
rg -n "fn commit_decision|expected_rev|lease_id|append.*record|transition_record" src/adapter/surreal
cargo test
```

### 중단 조건
- 이 점검이 adapter 전체 재작성으로 번지면 중단
- store abstraction을 다시 크게 바꾸려 하면 중단

---

## Phase 5 — preview release evidence pack

### 목표
내부 preview 운영에 필요한 release evidence를 파일로 남긴다.

### 변경 파일
- `scripts/verify-release.sh`
- 새 `scripts/export-release-evidence.sh` (필요 시)
- `README.md` 또는 release section
- `plans/TASKS.md`

### 작업
1. `.axiomnexus/releases/<version>/` 경로를 기준 evidence pack으로 고정한다.
2. 아래를 저장한다.
   - `verify-release.log`
   - `smoke-runtime.log`
   - `replay.log`
   - `store_snapshot.json`
   - 필요 시 `release-notes.md`
3. 내부 preview 태그 전 최소 evidence bundle을 남긴다.

### 완료 조건
- “이 릴리스를 왜 통과시켰는가?”를 파일로 설명할 수 있다.
- 다음 사람이 같은 절차를 재현할 수 있다.

### 검증 명령
```bash
scripts/verify-release.sh
ls -R .axiomnexus/releases
```

### 중단 조건
- evidence pack 자동화를 release pipeline 전체 자동화로 확대하려 하면 중단

---

## 5. 가장 추천하는 실행 순서

아래 순서 하나만 추천한다.

1. Phase 1 — 제품 정체성 잠금
2. Phase 2 — operator path 통일
3. Phase 3 — release gate 직접 증거 강화
4. Phase 4 — HTTP purity / Surreal commit contract 점검
5. Phase 5 — preview release evidence pack

왜 이 순서냐면:
- 먼저 “무엇을 만드는지”를 잠가야 잘못된 구현 확장이 멈춘다.
- 그 다음 “어떻게 쓰는지”를 통일해야 preview 운영이 가능해진다.
- 그 다음 “정말 맞게 동작하는지”를 증거로 확인해야 한다.
- 마지막에 운영 증거를 남기면 내부 사용이 시작된다.

---

## 6. 이번 버전 완료 기준

이번 버전은 아래가 모두 성립하면 끝이다.

1. README가 제품 정체성과 operator path를 분명히 설명한다.
2. `scheduler once` 기준 preview workflow가 실제로 통한다.
3. smoke/verify가 accepted transition / record / revision / session / consumption / replay를 직접 검증한다.
4. HTTP handler는 business rule을 직접 가지지 않는다.
5. Surreal adapter는 `commit_decision` 의미론을 문서 계약대로 지킨다.
6. preview release evidence pack이 남는다.

여기까지가 이번 버전의 제품 수준 완료선이다.
