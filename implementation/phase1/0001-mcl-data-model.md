# 0001 — MCL data model (amc-state)

- **Phase:** 1
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

A typed Rust model of MCL canonical state — project, concept (+ per-type properties), recipe,
appearance, behavior — that serde-round-trips to JSON matching `schemas/0.1.0/` and serializes
deterministically.

## Background

`SPEC-004 §7`–§18 (documents & properties), §8 (concept base object), §4 (envelope). The model
is the in-memory form of what `amc-schema` validates.

## Dependencies

`phase0/0003` (schemas), `phase0/0002` (ids/hashing).

## Implementation Notes

- New module tree in `amc-state` (`model/`): `Project`, `Concept`, `ConceptType`, `Properties`
  (item/weapon/tool/food/block variants), `Recipe`, `Appearance`, `Behavior`, `Reference`,
  `Status`.
- `#[serde(deny_unknown_fields)]` where the schema is closed; keep field names identical to the
  schema so `serde_json::to_value` validates against `amc-schema`.
- Model values must serialize deterministically (rely on serde_json sorted keys + `amc-core`
  canonical hashing). No timestamps inside hashable state.

## Acceptance Tests

- Round-trip: model → `serde_json::Value` → `amc_schema::validate(..)` is `Ok` for the SPEC-004
  §8 concept and §15 recipe examples.
- A constructed weapon concept serializes with `max_stack_size: 1`.

## Benchmark Coverage

`COMP-STATE-*` (model serialization).

## Traceability

- Product requirement(s): `FR-002` (canonical state).
- MCL element(s): all document types.
- Component(s): `amc-state`.
