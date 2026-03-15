# 품질 게이트와 검증 전략

## 개발 기본 게이트

가장 먼저 보는 기본 검증은 아래 한 묶음이다.

```bash
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

같은 내용을 스크립트로도 실행할 수 있다.

```bash
scripts/verify-runtime.sh
```

## release gate

릴리스 직전에는 아래를 추가로 본다.

```bash
scripts/verify-release.sh
```

이 경로는 다음을 포함한다.

1. 기본 Rust 검증
2. schema gate 테스트
3. runtime smoke

## 무엇을 증명해야 하는가

### kernel
- 상태 전이 규칙이 contract와 맞는가
- replay가 snapshot을 복원하는가

### app
- context load, evidence assembly, commit 호출 순서가 유지되는가

### adapter
- store adapter가 같은 의미 규칙을 지키는가
- runtime adapter가 execute-turn 계약을 지키는가

### docs / schema
- canonical 문서 표면이 살아 있는가
- `samples/` schema 경로가 코드와 문서에 맞게 연결되는가

## 이후 강화 항목

아래는 현재 기본 게이트가 아니라 추가 강화 항목이다.

- dual-store conformance 확대
- benchmark baseline
- 운영 관찰성 audit
