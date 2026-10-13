# 0003 — MCL schema infrastructure (amc-schema)

- **Phase:** 0
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

Versioned JSON Schemas for the MCL documents and a loader/validator crate, proven against the
specification's own worked examples.

## Background

`SPEC-004` (MCL Specification) §4 envelope, §8 concept, §15 recipe, §17 appearance, §18
behavior, §19 operation, §31 transaction. `SPEC-005 §3` (schema validation is a Layer-1
deterministic test that runs on every commit).

## Dependencies

`phase0/0001`.

## Implementation Notes

- Schemas live under `schemas/0.1.0/` and are embedded into the crate at build time
  (`include_str!`) so validation needs no filesystem/network (`SPEC-001 NFR-001`).
- Use the `jsonschema` crate. Public API: `validate(document_type, &Value) -> Result<(),
  Vec<SchemaError>>`.
- MVP scope only (`SPEC-001 §7`): item, weapon, tool, food, block; recipe; appearance;
  behavior; transaction; operation envelope.

## Acceptance Tests

- Unit test validates the SPEC-004 §20 `create_concept` (Ruby Blade) example → valid.
- Unit test validates SPEC-004 §15 shaped recipe example → valid.
- Negative test: a weapon with `max_stack_size: 2` → invalid (`SPEC-004 §11`).

## Benchmark Coverage

`COMP-*` schema validation targets (`SPEC-005 §3`). Schema-valid output rate feeds `SPEC-005
§20` gate (≥ 99%).

## Traceability

- Product requirement(s): `FR-004` (deterministic generation contract), `FR-002` (canonical state).
- MCL element(s): all document types.
- Component(s): `amc-schema`.
