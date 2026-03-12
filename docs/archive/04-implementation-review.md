> Archived on 2026-03-12. 이 문서는 날짜 고정 implementation review record이며 active architecture contract가 아닙니다. 현재 진입 문서는 [../00-index.md](../00-index.md) 입니다.

# 구현 검토

기준 시점: 2026-03-12  
검토 대상: 현재 worktree 전체

## 결론

현재 구현은 **재정렬 계획 P0-P5 기준으로 pass** 입니다.

핵심 증거:

- build gate가 모두 통과합니다.
- `commit_decision`이 rev/lease CAS를 저장 시점에 다시 검증합니다.
- timeout/reaper 경로도 `TransitionRecord`를 남기며 replay 가능합니다.
- `TransitionRecord`는 `reasons`, `before_status`, `after_status`, `evidence`를 보존합니다.
- `app`, `port`, `adapter` 경계는 현재 책임과 더 가깝게 정리됐습니다.

잔여 사항은 구조 blocker가 아니라 운영 메모 수준입니다.

- publication용 git remote는 아직 없습니다.
- HTTP surface는 v1 핵심보다 여전히 넓지만 현재 설계 위반은 아닙니다.

판정: **pass**

## 검증 결과

- `cargo fmt --all --check`: pass
- `cargo clippy --all-targets --all-features -- -D warnings`: pass
- `cargo test`: pass (`151 passed`)

## 현재 상태 요약

### asset / schema contract

- canonical runtime asset path는 `.agents/AGENTS.md`, `.agents/skills/transition-executor/SKILL.md`, `samples/transition-intent.schema.json`로 고정됐습니다.
- memory/surreal demo bootstrap은 `samples/company-rust-contract.example.json`를 사용합니다.
- `README`, boot, runtime, tests는 같은 schema path를 참조하도록 맞췄습니다.

### authoritative commit

- memory/surreal store 모두 `commit_decision`에서 `expected_rev`와 live lease를 다시 검사합니다.
- stale rev와 stale lease는 저장 단계에서 거절됩니다.
- claim은 split mutation이 아니라 authoritative commit 경로로 닫힙니다.

### explanation source

- `TransitionRecord`는 이제 `reasons`, `before_status`, `after_status`, `evidence`를 저장합니다.
- activity projection은 record의 authoritative status를 사용합니다.
- reject/conflict 이유와 timeout path까지 ledger에서 복원 가능합니다.

### IDC 일관성

- reaper는 direct snapshot mutation 대신 `TimeoutRequeue` system transition을 남깁니다.
- replay helper는 timeout/requeue chain까지 live snapshot과 일치하게 재구성합니다.

### 경계 정리

- `submit_intent`의 process/git I/O는 `WorkspacePort` 밖으로 이동했습니다.
- command-side actor/company fact load는 query projection 대신 authoritative read를 사용합니다.
- `app::qry` stub layer는 제거했습니다.
- `StorePort`는 역할별 trait(`CommandStorePort`, `RuntimeStorePort`, `SchedulerStorePort`, `QueryStorePort`)로 좁혀서 사용합니다.
- `RuntimePort` public surface는 coclai-only 현실에 맞게 `execute_turn` 하나로 단순화했습니다.
- 쓰이지 않던 `ClockPort`, `BlobPort`는 제거했습니다.

## 파일별 판정

### crate / boot

| 파일 | 판정 | 메모 |
| --- | --- | --- |
| `src/lib.rs` | aligned | 상위 모듈 구조와 canonical/invariant guard를 같이 검사합니다. |
| `src/boot/wire.rs` | aligned | live wiring과 contract check가 current store/runtime reality를 반영합니다. |

### model / kernel

| 파일 | 판정 | 메모 |
| --- | --- | --- |
| `src/model/transition.rs` | aligned | explanation source에 필요한 필드를 보존합니다. |
| `src/kernel/decide.rs` | aligned | 현재 저장소에서 핵심 IDC 규칙을 가장 잘 드러냅니다. |
| `src/kernel/replay.rs` | aligned | append-only record로 snapshot 재구성을 검증 가능한 형태로 고정합니다. |

### app

| 파일 | 판정 | 메모 |
| --- | --- | --- |
| `src/app/cmd/submit_intent.rs` | aligned | orchestration + evidence collection 요청만 맡고 process I/O는 port 밖입니다. |
| `src/app/cmd/claim_work.rs` | aligned | synthetic intent와 authoritative fact load를 사용해 하나의 commit 경로로 닫습니다. |
| `src/app/cmd/run_scheduler.rs` | aligned | scheduler는 queue 선택 orchestration만 하고 policy source는 분리됐습니다. |
| `src/app/cmd/resume_session.rs` | aligned | coclai-only runtime surface와 current session continuity에 맞습니다. |

### port / adapter

| 파일 | 판정 | 메모 |
| --- | --- | --- |
| `src/port/store.rs` | aligned | aggregate trait는 남지만 실제 call site는 narrower role trait를 사용합니다. |
| `src/port/runtime.rs` | aligned | coclai-only public surface만 남았습니다. |
| `src/adapter/http/transport.rs` | aligned | GET read-model과 command orchestration을 current boundary에 맞게 비춥니다. |
| `src/adapter/coclai/runtime.rs` | aligned | public port는 단순하고, start/resume/result는 adapter 내부 detail입니다. |
| `src/adapter/memory/store.rs` | partial | test seam이면서 store logic가 많지만 현재 v1 invariant는 지킵니다. |
| `src/adapter/surreal/store.rs` | partial | live store logic가 크지만 current v1 invariant는 지킵니다. |

## 남은 관찰

- memory/surreal store는 여전히 코드량이 크고 반복이 있습니다.
- HTTP product surface는 control plane 핵심보다 조금 넓습니다.
- 이 둘은 다음 최적화/단순화 대상이지 현재 재정렬 blocker는 아닙니다.
