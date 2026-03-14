# 최종 API 표면

## 원칙

외부 표면은 작아야 하지만, 운영자와 runtime control-plane에 필요한 표면은 빠지면 안 된다.  
따라서 표면을 **bootstrap / runtime control / observability** 세 묶음으로 나눈다.

---

## CLI

현재 canonical operator path는 HTTP execute endpoint가 아니라 CLI `scheduler once`다. [R1]
`run once <run_id>`는 특정 queued run을 직접 태우는 deterministic diagnostic path다.

### bootstrap / maintenance
- `migrate`
- `doctor`
- `contract check`
- `export`
- `import`

### runtime / serving
- `serve`
- `scheduler once` *(canonical operator path)*
- `run once <run_id>` *(diagnostic path)*
- `replay`

### 이후에도 추가하지 않을 것
- generic workflow builder CLI
- repo-local triad management CLI
- runtime plugin manager CLI

---

## HTTP Query

### bootstrap / admin read
- `GET /api/companies`
- `GET /api/contracts/active`
- `GET /api/agents`

### work / run read
- `GET /api/work`
- `GET /api/work/{id}`
- `GET /api/runs/{id}`
- `GET /api/board`
- `GET /api/activity`

### rationale
- board / activity / work detail는 read model이다. [R2][R3]
- query layer는 business rule을 새로 만들지 않는다. [R2]

---

## HTTP Command

### bootstrap / admin write
- `POST /api/companies`
- `POST /api/contracts`
- `POST /api/contracts/{id}/activate`
- `POST /api/agents`
- `POST /api/agents/{id}/pause`
- `POST /api/agents/{id}/resume`
- `POST /api/work`
- `POST /api/work/{id}/edit`

### runtime control
- `POST /api/work/{id}/queue`
- `POST /api/work/{id}/wake`
- `POST /api/work/{id}/reopen`
- `POST /api/work/{id}/cancel`
- `POST /api/work/{id}/override`
- `POST /api/work/{id}/intents`

여기서 queued run을 운영자가 소비시키는 canonical operator path는 `cargo run -- scheduler once`이고, 배포 검증이나 재현용 deterministic diagnostic path는 `cargo run -- run once <run_id>`다.  
핵심 write authority는 결국 `/api/work/{id}/intents`와 `commit_decision` 흐름이다.  
다른 command도 최종적으로는 동일한 decision path 또는 같은 store semantics로 수렴해야 한다. [R1][R2]

---

## SSE

axum은 HTTP/SSE surface를 구현하기에 충분하다. [AX1][AX2]

최종 event 종류:
- `lease-acquired`
- `lease-released`
- `wake-merged`
- `transition-accepted`
- `transition-rejected`
- `transition-conflict`
- `session-resumed`
- `session-reset`
- `run-running`
- `run-completed`
- `run-failed`

### keep-alive
SSE stream은 `KeepAlive`를 사용해 끊김을 줄인다. [AX2]

---

## API에서 하지 말 것

- HTTP handler 내부에서 transition rule 재작성
- transport DTO가 kernel 규칙을 대체
- runtime-specific branching이 route layer까지 새어나오기
- triad 내부 상태를 AxiomNexus HTTP surface에 그대로 노출

---

## 최종 surface에 대한 판단

현재 저장소는 company / contract / agent / work / run / activity surface를 이미 노출 대상으로 본다. [R1][R3]  
최종형에서도 이를 유지하되, **핵심 complexity는 오직 work transition kernel**에만 둔다.

즉 public surface가 조금 넓더라도, 진짜 복잡도는 커널 밖으로 새면 안 된다.
