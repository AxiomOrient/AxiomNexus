# COMMIT-PLAN

## Stage 1 — Surface Cleanup

### C01
docs: make root README and docs index the only canonical reader path

### C02
docs: sync canonical docs to actual CLI and release surface

### C03
docs: remove stale execute endpoint and dead release-script references

### C04
docs: record already-absent legacy files as closed cleanup items

### C05
docs: add canonical release checklist and notes template

## Stage 2 — Runtime E2E Gate

### C06
feat(runtime): add canonical scheduler-once auto path

### C07
test(smoke): add canonical runtime execute path to smoke

### C08
test(smoke): assert accepted transition updates snapshot, record, session, consumption, replay evidence

### C09
test(runtime): add invalid-session repair smoke or integration gate

## Stage 3 — Release Gate Split

### C10
docs(quality): split ship-now gates from later hardening gates

### C11
build(scripts): align verify script with ship-now release gate

### C12
test(schema): make schema drift a first-class release gate

## Stage 4 — Release Pack

### C13
docs(release): add release checklist and release note template

### C14
docs(release): document rollback and evidence preservation

## Stage 5 — Stable Backlog

### C15
plan: separate PostgreSQL adapter and dual-store conformance as stable backlog
