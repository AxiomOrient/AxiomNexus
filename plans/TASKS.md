# TASKS.md

상태:
- todo
- doing
- done
- defer

---

## Phase 1 — 제품 정체성 잠금

### NX-301
README 한 문장 정의 수정
목표:
- “심판 + 기록관”이 아니라
  “AI 소프트웨어 팀 업무 control plane”으로 설명
검증:
- README 첫 문단에서 제품 정체성이 명확
상태: done

### NX-302
README quick start를 운영 흐름 기준으로 재작성
목표:
- serve → queue → scheduler once → activity/replay 흐름 제시
검증:
- quick start만 봐도 내부 preview 사용 흐름 이해 가능
상태: done

### NX-303
`scheduler once` / `run once` 역할 문구 통일
목표:
- scheduler once = canonical operator path
- run once = deterministic diagnostic path
검증:
- README / CLI help / script logs 용어 일치
상태: done

### NX-304
“무엇을 하지 않는가” 섹션 짧게 추가
목표:
- goals/budgets/org chart/company OS 아님을 명확히
검증:
- scope drift 방지 문구 존재
상태: done

---

## Phase 2 — 실제 사용 흐름 완성

### NX-305
canonical preview workflow 문서화
목표:
- company → contract → agent → work → queue → scheduler once → replay
검증:
- 운영자가 실제 사용 순서를 이해 가능
상태: done

### NX-306
실사용 예시를 README 또는 docs index에서 링크
목표:
- “언제 어떻게 쓰는가?” 질문에 바로 답함
검증:
- entrypoint 문서에서 실사용 흐름 접근 가능
상태: done

### NX-307
smoke script 단계 이름을 실제 운영 흐름에 맞춤
목표:
- 스크립트 로그가 제품 사용 흐름과 같은 언어를 사용
검증:
- 로그만 봐도 현재 어느 운영 단계인지 이해 가능
상태: done

---

## Phase 3 — release evidence direct assertion

### NX-308
accepted transition direct assertion
목표:
- accepted complete 직접 확인
검증:
- smoke/verify output에 명시적 accepted evidence
상태: done

### NX-309
TransitionRecord append direct assertion
목표:
- append-only ledger 기록 직접 확인
검증:
- latest record 또는 record count 확인
상태: done

### NX-310
WorkSnapshot revision 증가 direct assertion
목표:
- commit 후 rev 증가 직접 확인
검증:
- before/after rev 비교
상태: done

### NX-311
task_session persistence direct assertion
목표:
- session continuity가 실제 저장되는지 확인
검증:
- session 정보 조회 가능
상태: done

### NX-312
consumption 기록 direct assertion
목표:
- token/cost/turn summary 확인
검증:
- agent 또는 consumption summary에서 값 확인
상태: done

### NX-313
replay pass assertion 강화
목표:
- replay가 integrity gate로 분명히 작동
검증:
- replay success가 verify-release에서 명시 출력
상태: done

---

## Phase 4 — preview release

### NX-314
verify-release 실행 절차 정리
목표:
- preview release 전 한 번의 절차로 검증 가능
검증:
- 운영자가 명령 한 줄로 release gate 수행 가능
상태: done

### NX-315
preview release evidence pack 저장
목표:
- smoke log / verify log / replay log / export snapshot 저장
검증:
- `.axiomnexus/releases/<version>/`에 산출물 존재
상태: done

### NX-316
preview 사용 가이드 확정
목표:
- 내부 팀이 실제 운영 시작 가능
검증:
- 최소 사용자 가이드 존재
상태: done

---

## Defer — 다음 버전

### NX-317
PostgreSQL adapter
상태: defer

### NX-318
dual-store conformance
상태: defer

### NX-319
benchmark baseline
상태: defer

### NX-320
observability hardening
상태: defer
