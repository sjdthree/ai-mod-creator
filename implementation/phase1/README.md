# Phase 1 — Canonical State (Weeks 2–3)

**Gate (`SPEC-006`):** create/update/delete operations succeed; rollback restores prior verified
state; state hashes are deterministic. Depends on Phase 0.

## Work items

| Item | Title | Effort | Status |
|---|---|---|---|
| [0001](./0001-mcl-data-model.md) | MCL data model (`mce-state`) | Medium | ✅ Done |
| [0002](./0002-reference-resolution.md) | Reference resolution | Medium | ✅ Done |
| [0003](./0003-operation-validator.md) | Operation validator (stages 1–6) | Large | ✅ Done |
| [0004](./0004-transactions.md) | Transactions, candidate state & commit | Large | ✅ Done |
| [0005](./0005-rollback.md) | Rollback (`revert_change`) | Small | ✅ Done |
| [0006](./0006-verification-requirements.md) | Verification-requirement generation | Medium | ✅ Done |
| [0007](./0007-persistence-sqlite.md) | Persistent state store (SQLite) | Medium | Not started |

Benchmark focus: `COMP-REF-*`, `COMP-STATE-*` (`SPEC-005 §3`).

> **Follow-on (`0007`):** state is currently **in-memory**. Per the research
> (internal research review), the transactional store should be
> **SQLite** (with canonical JSON document bodies) so projects persist across restarts.

**Phase 1 complete.** create/update/delete succeed ✅ · rollback restores prior verified state
✅ · state hashes deterministic ✅. Validator owns stages 1–6 (JSON → schema → references →
properties → semantics → capability); stages 7–8 (candidate-state schema + referential integrity)
are enforced by the state manager during candidate construction. Every mutating transaction
carries verification requirements (`SPEC-004 §32`).

## Suggested order

`0001` → `0002` → `0004` → `0005` → `0006` → `0003`. The data model and transactions unblock the
most; the full 8-stage validator (`0003`) can land last since early slices use direct schema
validation via `mce-schema`.
