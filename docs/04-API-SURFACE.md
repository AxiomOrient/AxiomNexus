# 최종 API 표면

## 원칙

외부 표면은 작게 유지하되, 운영에 필요한 읽기와 제어는 빠뜨리지 않는다.

## CLI

### bootstrap / maintenance
- `migrate`
- `doctor`
- `contract check`
- `export`
- `import`
- `replay`

### runtime / serving
- `serve`
- `scheduler once`
- `run once <run_id>`

### 의미
- `scheduler once`는 운영 기준 경로다
- `run once <run_id>`는 재현용 진단 경로다

## HTTP Query

- `GET /api/companies`
- `GET /api/contracts/active`
- `GET /api/agents`
- `GET /api/work`
- `GET /api/work/{id}`
- `GET /api/runs/{id}`
- `GET /api/board`
- `GET /api/activity`

## HTTP Command

### 운영자 write
- `POST /api/companies`
- `POST /api/contracts`
- `POST /api/contracts/{id}/activate`
- `POST /api/agents`
- `POST /api/agents/{id}/pause`
- `POST /api/agents/{id}/resume`
- `POST /api/work`
- `POST /api/work/{id}/edit`

### 상태 제어
- `POST /api/work/{id}/queue`
- `POST /api/work/{id}/wake`
- `POST /api/work/{id}/reopen`
- `POST /api/work/{id}/cancel`
- `POST /api/work/{id}/override`
- `POST /api/work/{id}/intents`

## SSE

- `GET /api/events`

이 표면은 after-commit 이벤트만 내보낸다.

## 표면 규칙

1. HTTP handler는 규칙을 새로 만들지 않는다.
2. authoritative write path는 항상 `Intent -> Decide -> Commit`으로 수렴한다.
3. query 표면은 운영 상태를 비추기만 한다.
