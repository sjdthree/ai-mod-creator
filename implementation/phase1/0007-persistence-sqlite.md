# 0007 — Persistent state store (SQLite)

- **Phase:** 1 (follow-on)
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** not started

## Objective

Persist canonical state, transaction history, and stable IDs to disk. The Phase 1 state manager is
in-memory only; a real project must survive restarts and support history/rollback across sessions.

## Background

Research (internal research review): storage format = **canonical JSON
+ SQLite** for history/transactions/IDs/metadata; YAML export optional; generated Java/resources
are disposable. `SPEC-004 §31` (transactions), `SPEC-001 FR-006` (rollback).

## Dependencies

`phase1/0004` (transactions), `phase1/0005` (rollback).

## Implementation Notes

- SQLite as the transactional system of record: transactions, normalized operations, state hashes,
  stable IDs, verification-requirement records, project metadata. Canonical **JSON** for document
  bodies (validation/patches/hashing/migrations).
- `project.mce.json` (or the DB) is the permanent source of truth; `generated/` is overwritten each
  run; `extensions/` (custom behavior, later) is never overwritten.
- Keep the deterministic `state_hash` unchanged; timestamps stay out of the hash.

## Acceptance Tests

- Create → commit → reopen: canonical state and history reload identically (same `state_hash`).
- Rollback works across a restart.

## Traceability

- Product requirement(s): `FR-002`, `FR-006`, `NFR-003`.
- MCL element(s): transaction `SPEC-004 §31`.
- Component(s): `mce-state`.
