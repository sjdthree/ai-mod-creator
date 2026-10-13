# 0002 — Reference resolution

- **Phase:** 1
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

Resolve intent-stage references (display name, alias, contextual selector, namespace id, vanilla
id) to stable concept IDs before any state mutation.

## Background

`SPEC-004 §6` (references), §34 (normalization), §5 (identifiers). `SPEC-005 §3` example
`COMP-REF-001`.

## Dependencies

`phase1/0001`.

## Implementation Notes

- `resolve(&state, &Reference) -> Result<Resolution>` where `Resolution` is either a unique
  stable id or a `NeedsClarification` (zero or multiple matches, `SPEC-004 §6`).
- Supported selectors: `last_created_concept`, `last_modified_concept`, `only_weapon`,
  `only_block`, `only_item`. Selector resolution reads ordering metadata tracked by the state
  manager.
- `minecraft_id` references pass through after resource-location validation; they are not
  project concepts.
- Never guess: ambiguous or missing → clarification, never a silent pick.

## Acceptance Tests

- `COMP-REF-001`: one concept named "Ruby Blade"; a `display_name` reference resolves to its id.
- Two concepts sharing an alias → `NeedsClarification`.
- `only_weapon` with exactly one weapon resolves; with two → clarification.

## Benchmark Coverage

`COMP-REF-*`.

## Traceability

- Product requirement(s): `FR-003` (incremental modification).
- MCL element(s): references (`SPEC-004 §6`), selectors.
- Component(s): `amc-state`.
