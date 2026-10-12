# 0005 — Rollback (revert_change)

- **Phase:** 1
- **Workstream:** core-engine
- **Effort:** Small
- **Owner:** unassigned
- **Status:** done

## Objective

Restore a previously verified transaction's state as a new transaction, without deleting history.

## Background

`SPEC-004 §30` (revert creates a new transaction; reverted state must be regenerated and verified
before commit), `SPEC-001 FR-006`.

## Dependencies

`phase1/0004`.

## Implementation Notes

- `revert(target_tx)` reconstructs canonical state as of `target_tx` and opens it as a **new**
  candidate transaction (parent = current head). It does not truncate the log.
- The reverted candidate must go through generation + verification before commit (wired in
  Phase 5); in Phase 1 it produces a candidate + deterministic hash.

## Acceptance Tests

- After create (tx1) + update (tx2), `revert(tx1)` yields a candidate whose state hash equals
  tx1's committed state hash.
- History still contains tx1 and tx2.

## Benchmark Coverage

`COMP-STATE-*` rollback; later `E2E-ROLLBACK-*`.

## Traceability

- Product requirement(s): `FR-006`.
- MCL element(s): `revert_change` (`SPEC-004 §30`), transaction statuses (§31).
- Component(s): `mce-state`.
