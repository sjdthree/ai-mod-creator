# 0004 — Transactions, candidate state & commit

- **Phase:** 1
- **Workstream:** core-engine
- **Effort:** Large
- **Owner:** unassigned
- **Status:** done

## Objective

The transactional heart of the state manager: apply validated operations to a candidate state,
assign stable IDs and namespace IDs, compute deterministic state hashes, and commit a candidate
to canonical **only after verification succeeds**.

## Background

`SPEC-004 §31` (transaction document & statuses), §5 (identifiers), §20–§28 (operations), §21
(update merge semantics), §23 (delete dependency policy), `SPEC-003 §8` (state manager). Failure
preservation: `SPEC-001 NFR-003`.

## Dependencies

`phase1/0001`, `phase1/0002`.

## Implementation Notes

- `StateManager` (real impl) owns canonical `Project` state + a transaction log.
- `open_candidate(validated_ops)` clones canonical state, applies ops (create/update/delete),
  assigns `concept-<ULID>` ids and `namespace:path` from `namespace_hint`/display name, records a
  `proposed`/`validated` transaction with `input_state_hash` and `candidate_state_hash`.
- `commit(tx, verified)` promotes candidate → canonical **only** when `verified`; otherwise the
  transaction is marked `verification_failed`/`build_failed` and canonical state is untouched.
- Update is merge-based (`SPEC-004 §21`); delete defaults to `reject_if_referenced`
  (`SPEC-004 §23`).
- Hashes exclude timestamps (`amc-core::canonical_hash`); same parent + same normalized ops →
  same `candidate_state_hash` (`SPEC-005 §22`).

## Acceptance Tests

- Create → candidate contains the concept with a stable id; commit(verified=true) makes it
  canonical.
- commit(verified=false) leaves canonical state byte-identical (hash unchanged).
- Same ops applied to same parent twice → identical `candidate_state_hash` (determinism).
- Delete of a referenced concept under `reject_if_referenced` fails without mutating state.

## Benchmark Coverage

`COMP-STATE-004` (failed candidate leaves parent unchanged), `COMP-GEN-009` precursor
(deterministic hash).

## Traceability

- Product requirement(s): `FR-002`, `FR-006` (rollback), `FR-007` (repeatability), `NFR-003`.
- MCL element(s): transaction (`SPEC-004 §31`), operations.
- Component(s): `amc-state`.
