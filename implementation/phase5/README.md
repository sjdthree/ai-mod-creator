# Phase 5 — End-to-End Loop (Weeks 10–11)

**Gate (`SPEC-006`):** the sequence below succeeds with no manual edits. Depends on Phases 3 & 4.

```bash
amc init ruby_world
amc request "Create a ruby sword called Ruby Blade with 1200 durability."
amc test
amc play
```

Anticipated work items:

- `0001` Wire CLI → Intent → Validator → State → Generator → Build → Verify → Launch.
- `0002` End-to-end trace preservation (`SPEC-004 §41`, `SPEC-005 §27`).
- `0003` Failure-preservation: a failed request leaves the last verified state intact (`SPEC-001 NFR-003`).
- `0004` Integration test suite across component boundaries (`SPEC-005 §11`).

Benchmark focus: `E2E-*` create/modify/reference/clarification/unsupported/rollback.
