# RELEASE-CHECKLIST

## Scope

이 체크리스트는 **preview / dogfood release**를 같은 절차로 재현하기 위한 canonical 문서다.
stable 추가 조건은 마지막 섹션에서 따로 본다.

## 1. Version and tag

- [ ] release version 확정
- [ ] git tag 이름 확정
- [ ] known limitations 정리

## 2. Ship-now gate

- [ ] `scripts/verify-release.sh`
- [ ] `cargo run -- export`

## 3. Runtime execute evidence

- [ ] queue → wake → `scheduler once` accepted complete 확인
- [ ] `TransitionRecord` append 확인
- [ ] `WorkSnapshot.rev` 증가 확인
- [ ] run status `completed` 확인
- [ ] `task_session` 저장/갱신 확인
- [ ] consumption 기록 확인
- [ ] replay pass 확인

## 4. Evidence pack

release evidence는 아래 경로에 고정한다.

```text
.axiomnexus/releases/<version>/
```

- [ ] `smoke-runtime.log`
- [ ] `verify-release.log`
- [ ] `replay.log`
- [ ] `store_snapshot.json`
- [ ] `release-notes.md`

## 5. Rollback

- [ ] `.axiomnexus/releases/<version>/store_snapshot.json` restore 경로 확인
- [ ] 이전 tag checkout 후 `cargo run -- import` 절차 확인
- [ ] contract / schema 호환성 주의사항 기록

## 6. Stable extra gate

아래는 preview blocker가 아니다.

- [ ] PostgreSQL adapter 완료
- [ ] dual-store conformance pass
- [ ] benchmark baseline 저장
- [ ] observability audit 완료
