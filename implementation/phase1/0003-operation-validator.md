# 0003 — Operation validator (8-stage)

- **Phase:** 1
- **Workstream:** core-engine
- **Effort:** Large
- **Owner:** unassigned
- **Status:** done

## Objective

Implement `mce-validator` running the ordered validation sequence that gates every operation
before it can mutate candidate state.

## Background

`SPEC-004 §33` (validation sequence), `SPEC-003 §7` (validator responsibilities & prohibitions),
`SPEC-004 §39` (generator capability registry), §35 (defaults), §36 (relative language policy —
relative terms must already be resolved by the adapter; the validator rejects unresolved words).

## Dependencies

`phase1/0001`, `phase1/0002`.

## Implementation Notes

- Stages, halting at first failure (`SPEC-004 §33`): JSON parse → operation schema (`mce-schema`)
  → reference resolution (`phase1/0002`) → property constraints → semantic constraints →
  capability constraints (generator registry) → candidate-state schema → referential integrity.
- Output: normalized operations + a validation report whose errors carry `code`, `operation`,
  `path`, `message`, `repairability`, `owning_component`, and optional clarification
  (`SPEC-004 §33`).
- Prohibited: no intent inference, no code generation, no silent correction of material intent,
  no commit.

## Acceptance Tests

- A valid `create_concept` normalizes and passes all stages.
- A recipe referencing a missing concept fails at reference resolution with the right owner.
- A capability-unsupported (but schema-valid) combination is rejected at the capability stage.

## Benchmark Coverage

`COMP-*` validation; feeds integration `INT-*` (intent+validation).

## Traceability

- Product requirement(s): `FR-002`, `NFR-003`.
- MCL element(s): operations (`SPEC-004 §19`), validation sequence (§33).
- Component(s): `mce-validator`, `mce-schema`, `mce-state`.
