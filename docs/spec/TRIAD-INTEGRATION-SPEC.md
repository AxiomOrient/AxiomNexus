# triad Integration Spec

## 결론

triad는 AxiomNexus 내부 모듈이 아니라 **외부 verification companion** 이다. [R1][T1]

---

## 1. 역할 분리

### AxiomNexus가 소유하는 것
- work lifecycle
- lease / wake / session
- transition decision
- transition ledger
- runtime orchestration

### triad가 소유하는 것
- claim verification
- evidence append log
- claim report computation [T1]

이 분리가 유지되어야 두 시스템이 서로를 잠식하지 않는다.

---

## 2. 왜 외부 companion이어야 하는가

triad README는 product surface를 고의로 매우 좁게 유지한다. [T1]

- pure verification core
- filesystem adapter
- thin CLI
- canonical work unit = `Claim`

이 성격은 AxiomNexus의 work lifecycle control plane과 다르다.  
따라서 repo-local workspace나 direct crate dependency보다 **file / CLI / JSON report bridge** 가 더 단순하다.

---

## 3. integration boundary

권장 경계는 아래 둘 중 하나다.

### A. CLI boundary
- `triad lint --json`
- `triad verify --claim ... --json`
- `triad report --claim ... --json` [T1]

### B. file boundary
- triad evidence / report JSON file를 읽어 `EvidenceBundle`로 변환

둘 다 괜찮지만, AxiomNexus core는 triad crate API에 직접 의존하지 않는다.

---

## 4. AxiomNexus에서 triad를 사용하는 방식

### 4.1 contract가 triad-backed proof를 요구할 수 있다
예:
- 특정 claim id가 `confirmed` 이어야 complete 허용
- `stale` / `unsupported` 면 reject
- `blocked` 면 block 또는 reject

### 4.2 triad 결과는 evidence input이다
triad 결과는 kernel decision의 입력일 뿐, state authority는 아니다.

### 4.3 triad artifact는 `EvidenceRef` 또는 `EvidenceInline` 로 남긴다
이렇게 해야 `TransitionRecord`에서 “왜 이런 판정이 났는지”를 설명할 수 있다. [R7]

---

## 5. mapping example

```text
triad confirmed      -> gate observation satisfied
triad contradicted   -> gate observation failed
triad blocked        -> gate observation blocked
triad stale          -> stale evidence
triad unsupported    -> missing support
```

최종 verdict는 여전히 kernel이 만든다.

---

## 6. 금지 규칙

- AxiomNexus main crate가 triad crate를 직접 import하지 않는다. [R1]
- triad가 AxiomNexus store를 직접 mutate하지 못한다.
- triad config / CLI surface를 AxiomNexus가 재노출하지 않는다.
- repo-local `.triad/*` bootstrap 자산을 canonical asset으로 만들지 않는다. [R1]

---

## 7. 완료 조건

1. triad result import가 `EvidenceBundle` 로 안정적으로 변환된다.
2. triad output은 `TransitionRecord` 에 evidence로 남는다.
3. triad absence/failure는 typed reason code로 드러난다.
4. direct dependency 없이 integration tests가 통과한다.
