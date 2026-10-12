# 0006 — Verification-requirement generation

- **Phase:** 1
- **Workstream:** core-engine
- **Effort:** Medium
- **Owner:** unassigned
- **Status:** done

## Objective

Every mutating operation automatically produces one or more verification requirements, so nothing
can be committed without a defined check.

## Background

`SPEC-004 §32` (verification requirement + MVP assertion types), `SPEC-005 §16` (generated
gameplay assertions per operation kind), `SPEC-003 §11` (harness consumes these).

## Dependencies

`phase1/0004`.

## Implementation Notes

- A `requirements_for(op, resolved_target)` function maps each mutating operation to required
  assertions, e.g. create weapon → `registry_exists`, `property_equals durability`,
  `property_equals attack_damage`, `weapon_damages`; add recipe → `recipe_exists`,
  `recipe_produces`; create block → `registry_exists`, `block_places`, `block_drops`;
  delete → `registry_absent`, `dependent_references_valid`.
- Requirements are attached to the transaction and carry a `verification_level`
  (`static`/`build`/`gametest`/...).
- The harness (Phase 3) may add mandatory derived tests; this task guarantees the minimum set.

## Acceptance Tests

- `create_concept` (weapon) yields ≥ the required weapon assertions.
- `add_recipe` yields `recipe_produces`.
- No mutating op yields an empty requirement set.

## Benchmark Coverage

Feeds Layer-4 `E2E-*`; unit `COMP-STATE-*` for requirement mapping.

## Traceability

- Product requirement(s): `FR-005` (automatic verification).
- MCL element(s): verification requirement (`SPEC-004 §32`), assertions.
- Component(s): `mce-state` (generation), `mce-verify` (consumption).
